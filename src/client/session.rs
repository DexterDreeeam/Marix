use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use marix_common::{
    ChannelEndpoint, Logger, NetReceiver, Receiver, Sender, SharedNetSender, build_channel,
    connect_channel,
};
use marix_protocol::{
    SessionEvent, SessionMessage, TaskEvent, TaskId, TaskRequest, TaskSignature, TaskStatus,
};

use crate::ClientEvent;

static SOURCE_NAME: OnceLock<String> = OnceLock::new();

const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(200);
const CONNECT_DIAGNOSTIC_INTERVAL: u64 = 25;
const CONNECTION_TERMINATED_SIGNATURE: &str = "__marix_client_connection__";
const CONNECTION_TERMINATED_MESSAGE: &str =
    "client connection event stream terminated before task completion";

pub struct ClientSession {
    state: Arc<ClientSessionState>,
    worker: Option<JoinHandle<()>>,
}

impl ClientSession {
    pub fn new(name: String) -> Self {
        let _ = SOURCE_NAME.set(name);
        let state = Arc::new(ClientSessionState::new());
        let worker = Self::spawn_worker(Arc::clone(&state));
        Self {
            state,
            worker: Some(worker),
        }
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }

    pub fn is_connected(&self) -> bool {
        self.state
            .server_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
    }

    pub fn create_task(
        &self,
        request: String,
        max_completion_time_secs: Option<u64>,
        max_relay_count: Option<u64>,
    ) {
        let signature = TaskSignature::new("task".to_owned());
        if self.send_to_server(SessionEvent::TaskCreate(TaskRequest {
            signature,
            content: request,
            max_completion_time_secs,
            max_relay_count,
        })) {
            Logger::log("client submitted task request");
        }
    }

    pub fn cancel_task(&self, task_id: TaskId) {
        let task_id_for_log = task_id.0.clone();
        let signature = TaskSignature {
            name: String::new(),
            id: task_id,
        };
        if self.send_to_server(SessionEvent::Task(signature, TaskEvent::Cancel)) {
            Logger::log(format!(
                "client submitted cancellation for task {task_id_for_log}"
            ));
        }
    }

    pub fn receiver(&self) -> &Receiver<ClientEvent> {
        &self.state.user_rx
    }

    pub fn close(&mut self) {
        self.state.shutdown.store(true, Ordering::Relaxed);
        let _ = self.worker.take();
    }
}

// -- Private -- //

impl ClientSession {
    fn spawn_worker(state: Arc<ClientSessionState>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let mut failed_attempts = 0_u64;
            while !state.shutdown.load(Ordering::Relaxed) {
                let (net_tx, net_rx) =
                    match connect_channel::<SessionMessage>(ChannelEndpoint::Client) {
                        Ok(channel) => channel,
                        Err(error) => {
                            failed_attempts = failed_attempts.saturating_add(1);
                            if failed_attempts == 1
                                || failed_attempts % CONNECT_DIAGNOSTIC_INTERVAL == 0
                            {
                                Logger::warning(format!(
                                    "client connection attempt \
                                     {failed_attempts} failed: {error:?}"
                                ));
                            }
                            std::thread::sleep(CONNECT_RETRY_DELAY);
                            continue;
                        }
                    };
                failed_attempts = 0;
                if state.shutdown.load(Ordering::Relaxed) {
                    break;
                }
                Logger::log("client connected to server core");
                *state
                    .server_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
                Self::worker(net_rx, &state.user_tx, &state.shutdown);
                *state
                    .server_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = None;
            }
        })
    }

    fn worker(
        mut server_rx: NetReceiver<SessionMessage>,
        user_tx: &Sender<ClientEvent>,
        shutdown: &AtomicBool,
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build client event runtime: {error}"));
        runtime.block_on(async move {
            loop {
                match server_rx.recv().await {
                    Ok(Some(message)) => {
                        if let Some(client_event) = Self::to_client_event(message.event) {
                            let _ = user_tx.send(client_event);
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        Logger::error(format!(
                            "client connection event stream failed: \
                             {error:?}"
                        ));
                        break;
                    }
                }
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
        if !shutdown.load(Ordering::Relaxed) {
            Logger::error(CONNECTION_TERMINATED_MESSAGE);
            let event = Self::done_event(
                CONNECTION_TERMINATED_SIGNATURE,
                Some(CONNECTION_TERMINATED_MESSAGE.to_owned()),
            );
            if let Err(error) = user_tx.send(event) {
                Logger::error(format!(
                    "client failed to report connection termination: \
                     {error}"
                ));
            }
        }
    }

    fn send_to_server(&self, event: SessionEvent) -> bool {
        let result = {
            let sender = self
                .state
                .server_tx
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            match sender.as_ref() {
                Some(sender) => sender
                    .try_send(Self::package_message(event))
                    .map(|_| ())
                    .map_err(|error| {
                        format!(
                            "client failed to send message to server core: \
                             {error}"
                        )
                    }),
                None => Err("client cannot send message: server connection is \
                     unavailable"
                    .to_owned()),
            }
        };
        if let Err(message) = result {
            Logger::error(message.clone());
            let event = Self::done_event(CONNECTION_TERMINATED_SIGNATURE, Some(message));
            if let Err(error) = self.state.user_tx.send(event) {
                Logger::error(format!("client failed to report send failure: {error}"));
            }
            return false;
        }
        true
    }

    fn to_client_event(event: SessionEvent) -> Option<ClientEvent> {
        match event {
            SessionEvent::SessionId(id) => {
                Logger::set_id(id);
                Logger::log("client session id updated");
                None
            }
            SessionEvent::TaskUpdate(TaskStatus::Succeed(result)) => {
                Some(Self::done_event("", Some(result.output)))
            }
            SessionEvent::TaskUpdate(TaskStatus::Failed { reason }) => {
                Some(Self::done_event("", Some(format!("task failed: {reason}"))))
            }
            SessionEvent::TaskUpdate(TaskStatus::Canceled) => Some(Self::done_event("", None)),
            SessionEvent::TaskUpdate(TaskStatus::Created) => {
                Some(Self::common_event("", "task created".to_owned()))
            }
            SessionEvent::TaskUpdate(TaskStatus::Started) => None,
            SessionEvent::ExecutorTools(_, _) => {
                Logger::warning("client session ignored executor tools event");
                None
            }
            _ => None,
        }
    }

    fn common_event(signature_id: impl Into<String>, message: String) -> ClientEvent {
        ClientEvent::Common {
            signature_id: signature_id.into(),
            message,
        }
    }

    fn done_event(signature_id: impl Into<String>, message: Option<String>) -> ClientEvent {
        ClientEvent::Done {
            signature_id: signature_id.into(),
            message,
        }
    }
}

struct ClientSessionState {
    server_tx: SharedNetSender<SessionMessage>,
    user_tx: Sender<ClientEvent>,
    user_rx: Receiver<ClientEvent>,
    shutdown: Arc<AtomicBool>,
}

impl ClientSessionState {
    fn new() -> Self {
        let (user_tx, user_rx) = build_channel();
        Self {
            server_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
            user_tx,
            user_rx,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }
}

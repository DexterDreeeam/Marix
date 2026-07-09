use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use marix_common::{
    ChannelEndpoint, Logger, NetReceiver, Receiver, Sender, SharedNetSender, build_channel,
    connect_channel,
};
use marix_protocol::{
    SessionEvent, SessionMessage, TaskEvent, TaskId, TaskRequest, TaskSignature, TaskStatus,
};

use crate::ClientEvent;

static SOURCE_NAME: OnceLock<String> = OnceLock::new();

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

    pub fn create_task(&self, request: String) {
        Logger::log("client submitting task request");
        let signature = TaskSignature::new("task".to_owned());
        self.send_to_server(SessionEvent::TaskCreate(TaskRequest {
            signature,
            content: request,
        }));
    }

    pub fn cancel_task(&self, task_id: TaskId) {
        Logger::log(format!("client canceling task {}", task_id.0));
        let signature = TaskSignature {
            name: String::new(),
            id: task_id,
        };
        self.send_to_server(SessionEvent::Task(signature, TaskEvent::Cancel));
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
            while !state.shutdown.load(Ordering::Relaxed) {
                let Ok((net_tx, net_rx)) =
                    connect_channel::<SessionMessage>(ChannelEndpoint::Client)
                else {
                    continue;
                };
                Logger::log("client connected to server core");
                *state
                    .server_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
                Self::worker(net_rx, &state.user_tx, &state.shutdown);
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
            while let Ok(Some(message)) = server_rx.recv().await {
                if let Some(client_event) = Self::to_client_event(message.event) {
                    let _ = user_tx.send(client_event);
                }
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }

    fn send_to_server(&self, event: SessionEvent) {
        if let Some(sender) = self
            .state
            .server_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(Self::package_message(event));
        }
    }

    fn to_client_event(event: SessionEvent) -> Option<ClientEvent> {
        match event {
            SessionEvent::TaskUpdate(TaskStatus::Succeed(result)) => {
                Some(Self::done_event("", Some(result.content)))
            }
            SessionEvent::TaskUpdate(TaskStatus::Failed { reason }) => {
                Some(Self::done_event("", Some(format!("task failed: {reason}"))))
            }
            SessionEvent::TaskUpdate(TaskStatus::Canceled) => Some(Self::done_event("", None)),
            SessionEvent::TaskUpdate(TaskStatus::Created) => {
                Some(Self::common_event("", "task created".to_owned()))
            }
            SessionEvent::TaskUpdate(TaskStatus::Started) => {
                Some(Self::common_event("", "task started".to_owned()))
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

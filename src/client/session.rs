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
    server_tx: SharedNetSender<SessionMessage>,
    user_rx: Receiver<ClientEvent>,
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
}

impl ClientSession {
    pub fn new(name: String) -> Self {
        let _ = SOURCE_NAME.set(name);
        let (user_tx, user_rx) = build_channel();
        let server_tx: SharedNetSender<SessionMessage> =
            SharedNetSender::new(std::sync::Mutex::new(None));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = Self::spawn_worker(Arc::clone(&server_tx), user_tx, Arc::clone(&shutdown));
        Self {
            server_tx,
            user_rx,
            worker: Some(worker),
            shutdown,
        }
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }

    pub fn is_connected(&self) -> bool {
        self.server_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
    }

    pub fn create_task(&self, request: String) {
        Logger::log("client submitting task request");
        self.send_to_server(SessionEvent::TaskCreate(TaskRequest { content: request }));
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
        &self.user_rx
    }

    pub fn close(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = self.worker.take();
    }
}

// -- Private -- //

impl ClientSession {
    fn spawn_worker(
        server_tx: SharedNetSender<SessionMessage>,
        user_tx: Sender<ClientEvent>,
        shutdown: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            while !shutdown.load(Ordering::Relaxed) {
                let Ok((net_tx, net_rx)) =
                    connect_channel::<SessionMessage>(ChannelEndpoint::Client)
                else {
                    continue;
                };
                Logger::log("client connected to server core");
                *server_tx.lock().unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
                Self::worker(net_rx, &user_tx, &shutdown);
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

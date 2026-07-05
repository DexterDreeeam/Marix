use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use marix_common::{
    Config, NetReceiver, Receiver, Sender, SharedNetSender, build_channel, connect_channel,
};
use marix_protocol::{
    SessionEvent, SessionMessage, StepEvent, TaskEvent, TaskId, TaskSignature, TaskStatus,
};

use crate::ClientEvent;

static SOURCE_NAME: OnceLock<String> = OnceLock::new();

pub struct ClientSession {
    agent_tx: SharedNetSender<SessionMessage>,
    user_rx: Receiver<ClientEvent>,
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
}

impl ClientSession {
    pub fn new(name: String) -> Self {
        let _ = SOURCE_NAME.set(name);
        let (user_tx, user_rx) = build_channel();
        let agent_tx: SharedNetSender<SessionMessage> =
            SharedNetSender::new(std::sync::Mutex::new(None));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = Self::spawn_worker(Arc::clone(&agent_tx), user_tx, Arc::clone(&shutdown));
        Self {
            agent_tx,
            user_rx,
            worker: Some(worker),
            shutdown,
        }
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }

    pub fn is_connected(&self) -> bool {
        self.agent_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some()
    }

    pub fn create_task(&self, request: String) {
        let signature = TaskSignature::new(request.clone());
        self.send_to_agent(SessionEvent::Task(signature, TaskEvent::Create { request }));
    }

    pub fn cancel_task(&self, task_id: TaskId) {
        let signature = TaskSignature {
            name: String::new(),
            id: task_id,
        };
        self.send_to_agent(SessionEvent::Task(signature, TaskEvent::Cancel));
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
        agent_tx: SharedNetSender<SessionMessage>,
        user_tx: Sender<ClientEvent>,
        shutdown: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address: SocketAddr = config
                .client
                .core_address
                .parse()
                .unwrap_or_else(|error| panic!("invalid core address: {error}"));
            while !shutdown.load(Ordering::Relaxed) {
                let Ok((net_tx, net_rx)) = connect_channel::<SessionMessage>(address) else {
                    continue;
                };
                *agent_tx.lock().unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
                Self::run_worker(net_rx, &user_tx, &shutdown);
            }
        })
    }

    fn run_worker(
        mut agent_rx: NetReceiver<SessionMessage>,
        user_tx: &Sender<ClientEvent>,
        shutdown: &AtomicBool,
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build client event runtime: {error}"));
        runtime.block_on(async move {
            while let Ok(Some(message)) = agent_rx.recv().await {
                if let Some(client_event) = Self::to_client_event(message.event) {
                    let _ = user_tx.send(client_event);
                }
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }

    fn send_to_agent(&self, event: SessionEvent) {
        if let Some(sender) = self
            .agent_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let _ = sender.try_send(Self::package_message(event));
        }
    }

    fn to_client_event(event: SessionEvent) -> Option<ClientEvent> {
        match event {
            SessionEvent::Task(_, TaskEvent::Status(TaskStatus::Update { content })) => {
                Some(ClientEvent::Common(content))
            }
            SessionEvent::Task(_, TaskEvent::Status(TaskStatus::Failed { reason })) => {
                Some(ClientEvent::Common(reason))
            }
            SessionEvent::Task(_, TaskEvent::Preview { content }) => {
                Some(ClientEvent::Common(content))
            }
            SessionEvent::Task(_, TaskEvent::CreateFailed { reason }) => {
                Some(ClientEvent::Common(reason))
            }
            SessionEvent::Step(_, StepEvent::Update { content, .. }) => {
                Some(ClientEvent::Common(content))
            }
            SessionEvent::Step(_, StepEvent::Complete { seq_count, result }) => {
                (seq_count == 0).then_some(ClientEvent::Common(result.content))
            }
            SessionEvent::Step(_, StepEvent::Fail { result }) => {
                Some(ClientEvent::Common(result.content))
            }
            _ => None,
        }
    }
}

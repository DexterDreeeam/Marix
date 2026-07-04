use std::net::SocketAddr;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::thread::{self, JoinHandle};

use marix_common::{
    Config, SessionEvent, SessionMessage, TaskEvent, TaskId, TaskSignature, accept_channel,
};

use super::SessionState;
use crate::task::Task;

static SOURCE_NAME: OnceLock<String> = OnceLock::new();

pub struct Session {
    pub state: Arc<SessionState>,
    client_worker: JoinHandle<()>,
    host_worker: JoinHandle<()>,
}

impl Session {
    pub fn new(name: String) -> Self {
        let _ = SOURCE_NAME.set(name);
        let state = Arc::new(SessionState::new());
        let client_worker = Arc::clone(&state).spawn_client_worker();
        let host_worker = Arc::clone(&state).spawn_host_worker();
        Self {
            state,
            client_worker,
            host_worker,
        }
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }
}

// -- Private -- //

impl SessionState {
    fn parse_address(address: &str, label: &str) -> SocketAddr {
        address
            .parse()
            .unwrap_or_else(|error| panic!("invalid {label} bind address: {error}"))
    }

    fn spawn_client_worker(self: Arc<Self>) -> JoinHandle<()> {
        thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address = Self::parse_address(&config.agent.client_bind_address, "client");
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(address) else {
                    continue;
                };
                *self
                    .client_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *self
                    .client_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::run_client_worker(Arc::clone(&self));
            }
        })
    }

    fn spawn_host_worker(self: Arc<Self>) -> JoinHandle<()> {
        thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address = Self::parse_address(&config.agent.host_bind_address, "host");
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(address) else {
                    continue;
                };
                *self
                    .host_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *self
                    .host_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::run_host_worker(Arc::clone(&self));
            }
        })
    }

    fn run_client_worker(state: Arc<Self>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build client event runtime: {error}"));
        runtime.block_on(async move {
            let Some(mut rx) = state
                .client_rx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            else {
                return;
            };
            while let Ok(Some(message)) = rx.recv().await {
                match message.event {
                    SessionEvent::Task(signature, TaskEvent::Create { request }) => {
                        state.create_task(signature, request);
                    }
                    event => state.route_session_event(event),
                }
            }
        });
    }

    fn run_host_worker(state: Arc<Self>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build host event runtime: {error}"));
        runtime.block_on(async move {
            let Some(mut rx) = state
                .host_rx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            else {
                return;
            };
            while let Ok(Some(message)) = rx.recv().await {
                state.route_session_event(message.event);
            }
        });
    }

    fn create_task(&self, mut signature: TaskSignature, request: String) {
        signature.name = request;
        if self
            .host_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_none()
        {
            let event = SessionEvent::Task(
                signature,
                TaskEvent::CreateFailed {
                    reason: "host not connected".to_string(),
                },
            );
            if let Some(sender) = self
                .client_tx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .as_mut()
            {
                let _ = sender.try_send(Session::package_message(event));
            }
            return;
        }
        let task_id = signature.id.clone();
        let task = Task::new(
            signature,
            Arc::clone(&self.client_tx),
            Arc::clone(&self.host_tx),
        );
        self.tasks.insert(task_id, Arc::new(StdMutex::new(task)));
    }

    fn route_session_event(&self, event: SessionEvent) {
        match &event {
            SessionEvent::Task(signature, _) => {
                self.route_task_event(&signature.id, event.clone());
            }
            SessionEvent::Execution(signature, _) => {
                self.route_task_event(&signature.task_id, event.clone());
            }
        }
    }

    fn route_task_event(&self, task_id: &TaskId, event: SessionEvent) {
        let task = self.tasks.get(task_id.clone());
        let sender = task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .sender();
        let _ = sender.send(event);
    }
}

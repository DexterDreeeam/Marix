use std::net::SocketAddr;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::thread::{self, JoinHandle};

use marix_common::{Config, accept_channel};
use marix_protocol::{
    ExecutionEvent, ExecutionSignature, SessionEvent, SessionMessage, TaskEvent, TaskId,
    TaskSignature,
};

use super::{SessionContext, SessionState};
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
        let client_worker = Self::spawn_client_worker(Arc::clone(&state));
        let host_worker = Self::spawn_host_worker(Arc::clone(&state));
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

impl Session {
    fn parse_address(address: &str, label: &str) -> SocketAddr {
        address
            .parse()
            .unwrap_or_else(|error| panic!("invalid {label} bind address: {error}"))
    }

    fn spawn_client_worker(state: Arc<SessionState>) -> JoinHandle<()> {
        thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address = Self::parse_address(&config.agent.client_bind_address, "client");
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(address) else {
                    continue;
                };
                *state
                    .client_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *state
                    .client_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::run_client_worker(Arc::clone(&state));
            }
        })
    }

    fn spawn_host_worker(state: Arc<SessionState>) -> JoinHandle<()> {
        thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address = Self::parse_address(&config.agent.host_bind_address, "host");
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(address) else {
                    continue;
                };
                *state
                    .host_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *state
                    .host_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::reset_session_context(&state);
                Self::query_host_preview(&state);
                Self::run_host_worker(Arc::clone(&state));
                Self::host_disconnect(&state);
            }
        })
    }

    fn run_client_worker(state: Arc<SessionState>) {
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
                        Self::create_task(&state, signature, request);
                    }
                    event => Self::route_session_event(&state, event),
                }
            }
        });
    }

    fn run_host_worker(state: Arc<SessionState>) {
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
                Self::route_session_event(&state, message.event);
            }
        });
    }

    fn create_task(state: &SessionState, mut signature: TaskSignature, request: String) {
        signature.name = request;
        if state
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
            if let Some(sender) = state
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
            Arc::clone(&state.context),
            signature,
            Arc::clone(&state.client_tx),
            Arc::clone(&state.host_tx),
        );
        state.tasks.insert(task_id, Arc::new(StdMutex::new(task)));
    }

    fn route_session_event(state: &SessionState, event: SessionEvent) {
        match &event {
            SessionEvent::Task(signature, _) => {
                Self::route_task_event(state, &signature.id, event.clone());
            }
            SessionEvent::Step(_, _) => {}
            SessionEvent::Execution(_, ExecutionEvent::Preview { system, tools }) => {
                *state
                    .host_sys
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(*system);
                state
                    .context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .tools = tools.clone();
            }
            SessionEvent::Execution(signature, _) => {
                Self::route_task_event(state, &signature.task_id, event.clone());
            }
        }
    }

    fn route_task_event(state: &SessionState, task_id: &TaskId, event: SessionEvent) {
        let task = state.tasks.get(task_id.clone());
        let sender = task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .sender();
        let _ = sender.send(event);
    }

    fn host_disconnect(state: &SessionState) {
        *state
            .client_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        *state
            .client_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        *state
            .host_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        *state
            .host_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        state.tasks.clear();
        *state
            .host_sys
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        Self::reset_session_context(state);
    }

    fn reset_session_context(state: &SessionState) {
        *state
            .context
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = SessionContext {
            tasks: Vec::new(),
            tools: Vec::new(),
        };
    }

    fn query_host_preview(state: &SessionState) {
        if let Some(sender) = state
            .host_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            let signature = ExecutionSignature::new(TaskId::new(), "preview".to_string());
            let _ = sender.try_send(Self::package_message(SessionEvent::Execution(
                signature,
                ExecutionEvent::PreviewQuery,
            )));
        }
    }
}

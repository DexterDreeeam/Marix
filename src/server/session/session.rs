use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::thread::{self, JoinHandle};

use marix_common::{ChannelEndpoint, Logger, accept_channel};
use marix_protocol::{
    ExecutorEvent, SessionEvent, SessionMessage, TaskId, TaskRequest, TaskSignature, TaskStatus,
};

use super::{SessionContext, SessionState};
use crate::task::Task;

static SOURCE_NAME: OnceLock<String> = OnceLock::new();

pub struct Session {
    pub state: Arc<SessionState>,
    _client_worker: JoinHandle<()>,
    _host_worker: JoinHandle<()>,
    _task_worker: JoinHandle<()>,
}

impl Session {
    pub fn new(name: String) -> Self {
        let _ = Logger::log(format!("core session '{name}' initializing"));
        let _ = SOURCE_NAME.set(name);
        let state = Arc::new(SessionState::new());
        let client_worker = Self::spawn_client_worker(Arc::clone(&state));
        let host_worker = Self::spawn_host_worker(Arc::clone(&state));
        let task_worker = Self::task_worker(Arc::clone(&state));
        Self {
            state,
            _client_worker: client_worker,
            _host_worker: host_worker,
            _task_worker: task_worker,
        }
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }
}

// -- Private -- //

impl Session {
    fn spawn_client_worker(state: Arc<SessionState>) -> JoinHandle<()> {
        thread::spawn(move || {
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(ChannelEndpoint::Client) else {
                    continue;
                };
                let _ = Logger::log("client channel connected");
                *state
                    .client_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *state
                    .client_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::client_worker(Arc::clone(&state));
            }
        })
    }

    fn spawn_host_worker(state: Arc<SessionState>) -> JoinHandle<()> {
        thread::spawn(move || {
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(ChannelEndpoint::Host) else {
                    continue;
                };
                let _ = Logger::log("host channel connected");
                *state
                    .host_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *state
                    .host_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::reset_session_context(&state);
                Self::host_worker(Arc::clone(&state));
                Self::host_disconnect(&state);
            }
        })
    }

    fn task_worker(state: Arc<SessionState>) -> JoinHandle<()> {
        thread::spawn(move || {
            loop {
                let event = state
                    .task_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .recv();
                let Ok(event) = event else {
                    break;
                };
                Self::dispatch(&state, event);
            }
        })
    }

    fn client_worker(state: Arc<SessionState>) {
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
                if state.task_tx.send(message.event).is_err() {
                    let _ = Logger::warning("session event enqueue failed: session worker stopped");
                }
            }
        });
    }

    fn host_worker(state: Arc<SessionState>) {
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
                if state.task_tx.send(message.event).is_err() {
                    let _ = Logger::warning("session event enqueue failed: session worker stopped");
                }
            }
        });
    }

    fn create_task(state: &SessionState, request: TaskRequest) {
        let signature = TaskSignature::new("task".to_owned());
        if state
            .host_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_none()
        {
            let reason = "host not connected".to_string();
            let _ = Logger::warning(format!("task {} rejected: {reason}", signature.id.0));
            Self::send_client_event(
                state,
                SessionEvent::TaskUpdate(TaskStatus::Failed { reason }),
            );
            return;
        }
        let task_id = signature.id.clone();
        let _ = Logger::log(format!("task {} created", task_id.0));
        let task = Task::new(
            Arc::clone(&state.context),
            signature,
            request.content,
            state.task_tx.clone(),
        );
        state.tasks.insert(task_id, Arc::new(StdMutex::new(task)));
    }

    fn dispatch(state: &SessionState, event: SessionEvent) {
        match &event {
            SessionEvent::TaskCreate(request) => {
                Self::create_task(state, request.clone());
            }
            SessionEvent::Task(signature, _) => {
                Self::dispatch_task(state, &signature.id, event.clone());
            }
            SessionEvent::TaskUpdate(_) => {
                Self::send_client_event(state, event);
            }
            SessionEvent::Executor(event) => {
                Self::send_host_event(state, SessionEvent::Executor(event.clone()));
            }
        }
    }

    fn dispatch_task(state: &SessionState, task_id: &TaskId, event: SessionEvent) {
        let Some(sent) = state.tasks.with(task_id, |task| {
            let sender = task
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .sender();
            sender.send(event).is_ok()
        }) else {
            let _ = Logger::warning(format!(
                "session could not dispatch event: task {} not found",
                task_id.0
            ));
            return;
        };
        if !sent {
            let _ = Logger::warning(format!(
                "session could not dispatch event: task {} worker stopped",
                task_id.0
            ));
        }
    }

    fn send_client_event(state: &SessionState, event: SessionEvent) {
        if !matches!(event, SessionEvent::TaskUpdate(_)) {
            return;
        }
        if let Some(sender) = state
            .client_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            if let Err(error) = sender.try_send(Session::package_message(event)) {
                let _ =
                    Logger::warning(format!("core session could not send client event: {error}"));
            }
        }
    }

    fn send_host_event(state: &SessionState, event: SessionEvent) {
        if !matches!(
            event,
            SessionEvent::Executor(ExecutorEvent::Execution(_, _))
                | SessionEvent::Executor(ExecutorEvent::ExecutionCreate(_))
                | SessionEvent::Executor(ExecutorEvent::ExecutionUpdate(_, _))
        ) {
            let _ = Logger::warning("core session ignored non-executor host event");
            return;
        }
        if let Some(sender) = state
            .host_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            if let Err(error) = sender.try_send(Session::package_message(event)) {
                let _ = Logger::warning(format!("core session could not send host event: {error}"));
            }
        } else {
            let _ = Logger::warning("core session could not send host event: host disconnected");
        }
    }

    fn host_disconnect(state: &SessionState) {
        let _ = Logger::warning("host disconnected; clearing session state");
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
            system: None,
            tasks: Vec::new(),
            tools: Vec::new(),
        };
    }
}

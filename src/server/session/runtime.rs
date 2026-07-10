use std::convert::Infallible;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::thread;

use marix_common::{
    ChannelEndpoint, Logger, Receiver, Sender, accept_channel, build_channel, select,
};
use marix_protocol::{
    Actor, ExecutorEvent, Runtime, SessionEvent, SessionMessage, TaskEvent, TaskRequest,
    TaskSignature, TaskStatus,
};

use super::{Session, SessionContext, SessionState};
use crate::task::Task;

pub struct SessionRuntime {
    state: Arc<SessionState>,
    close_tx: Sender<()>,
    close_rx: Receiver<()>,
}

impl SessionRuntime {
    pub fn new(state: Arc<SessionState>) -> Self {
        let (close_tx, close_rx) = build_channel();
        Self {
            state,
            close_tx,
            close_rx,
        }
    }
}

impl Runtime<SessionEvent, Infallible> for SessionRuntime {
    fn run(&self) {
        self.spawn_client_worker();
        self.spawn_host_worker();
        Logger::debug("core session runtime loop starting");
        loop {
            select! {
                recv(&self.close_rx) -> _ => break,
                recv(&self.state.session_rx) -> event => {
                    let Ok(event) = event else {
                        break;
                    };
                    if let Err(error) = self.dispatch(event) {
                        match error {}
                    }
                }
            }
        }
        Logger::debug("core session runtime loop stopped");
    }

    fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!("core session close signal failed: {error}"));
        }
    }

    fn dispatch(&self, event: SessionEvent) -> Result<(), Infallible> {
        match &event {
            SessionEvent::TaskCreate(request) => {
                self.create_task(request.clone());
            }
            SessionEvent::Task(signature, task_event) => {
                self.dispatch_task(signature, task_event.clone());
            }
            SessionEvent::TaskUpdate(_) => {
                self.send_client_event(event);
            }
            SessionEvent::Executor(event) => {
                self.send_host_event(SessionEvent::Executor(event.clone()));
            }
        }
        Ok(())
    }
}

// -- Private -- //

impl SessionRuntime {
    fn spawn_client_worker(&self) {
        let state = Arc::clone(&self.state);
        drop(thread::spawn(move || {
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(ChannelEndpoint::Client) else {
                    continue;
                };
                Logger::log("client channel connected");
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
        }));
    }

    fn spawn_host_worker(&self) {
        let state = Arc::clone(&self.state);
        drop(thread::spawn(move || {
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(ChannelEndpoint::Host) else {
                    continue;
                };
                Logger::log("host channel connected");
                *state
                    .host_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *state
                    .host_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::reset_context(&state);
                Self::host_worker(Arc::clone(&state));
                Self::host_disconnect(&state);
            }
        }));
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
                if state.session_tx.send(message.event).is_err() {
                    Logger::warning("session event enqueue failed: session worker stopped");
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
                if state.session_tx.send(message.event).is_err() {
                    Logger::warning("session event enqueue failed: session worker stopped");
                }
            }
        });
    }

    fn create_task(&self, request: TaskRequest) {
        let TaskRequest { signature, content } = request;
        if self
            .state
            .host_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_none()
        {
            let reason = "host not connected".to_string();
            Logger::warning(format!("task {signature} rejected: {reason}"));
            self.send_client_event(SessionEvent::TaskUpdate(TaskStatus::Failed { reason }));
            return;
        }
        Logger::log(format!("task {signature} created"));
        let task = Arc::new(StdMutex::new(Task::new(
            Arc::clone(&self.state.context),
            signature.clone(),
            content,
            self.state.session_tx.clone(),
        )));
        self.state
            .tasks
            .insert(signature.clone(), Arc::clone(&task));
        self.state.tasks.with_mut(&signature, |task| {
            task.lock()
                .unwrap_or_else(|error| error.into_inner())
                .start();
        });
    }

    fn dispatch_task(&self, signature: &TaskSignature, event: TaskEvent) {
        let mut event = Some(event);
        let Some(()) = self.state.tasks.with(signature, |task| {
            task.lock()
                .unwrap_or_else(|error| error.into_inner())
                .dispatch(event.take().unwrap_or_else(|| {
                    unreachable!("task event already dispatched")
                }))
        }) else {
            let event = event.unwrap_or_else(|| {
                unreachable!("task event dispatched without a task")
            });
            Logger::warning(format!(
                "session could not dispatch event {event:?}: task {signature} not found",
            ));
            return;
        };
    }

    fn send_client_event(&self, event: SessionEvent) {
        if !matches!(event, SessionEvent::TaskUpdate(_)) {
            return;
        }
        if let Some(sender) = self
            .state
            .client_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            if let Err(error) = sender.try_send(Session::package_message(event)) {
                Logger::warning(format!("core session could not send client event: {error}"));
            }
        }
    }

    fn send_host_event(&self, event: SessionEvent) {
        if !matches!(
            event,
            SessionEvent::Executor(ExecutorEvent::Execution(_, _))
                | SessionEvent::Executor(ExecutorEvent::ExecutionCreate(_))
                | SessionEvent::Executor(ExecutorEvent::ExecutionUpdate(_, _))
        ) {
            Logger::warning("core session ignored non-executor host event");
            return;
        }
        if let Some(sender) = self
            .state
            .host_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_mut()
        {
            if let Err(error) = sender.try_send(Session::package_message(event)) {
                Logger::warning(format!("core session could not send host event: {error}"));
            }
        } else {
            Logger::warning("core session could not send host event: host disconnected");
        }
    }

    fn host_disconnect(state: &SessionState) {
        Logger::warning("host disconnected; clearing session state");
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
        Self::reset_context(state);
    }

    fn reset_context(state: &SessionState) {
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

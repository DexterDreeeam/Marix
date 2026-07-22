use std::convert::Infallible;
use std::sync::Arc;
use std::thread;

use marix_common::{
    Actor, ChannelEndpoint, Logger, Receiver, Sender, System, WorkQueue, accept_channel,
    build_channel, select,
};
use marix_protocol::{
    ExecutorEvent, SessionEvent, SessionMessage, TaskEvent, TaskRequest, TaskSignature, TaskStatus,
    ToolPreview,
};

use super::{Session, SessionContext, SessionState};
use crate::task::Task;

#[derive(Clone)]
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

    pub fn run(&self) {
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

    pub fn close(&self) {
        if let Err(error) = self.close_tx.send(()) {
            Logger::warning(format!("core session close signal failed: {error}"));
        }
    }

    pub fn dispatch(&self, event: SessionEvent) -> Result<(), Infallible> {
        match event {
            SessionEvent::SessionId(_) => {
                Logger::warning("core session received unsupported session id event");
            }
            SessionEvent::TaskCreate(request) => {
                self.create_task(request);
            }
            SessionEvent::Task(signature, task_event) => {
                self.dispatch_task(&signature, task_event);
            }
            SessionEvent::TaskUpdate(status) => {
                self.send_client_event(SessionEvent::TaskUpdate(status));
            }
            SessionEvent::ExecutorTools(system, tools) => {
                self.register_executor_tools(system, tools);
            }
            SessionEvent::Executor(event) => {
                self.send_host_event(SessionEvent::Executor(event));
            }
        }
        Ok(())
    }
}

// -- Private -- //

impl SessionRuntime {
    fn spawn_client_worker(&self) {
        let runtime = self.clone();
        drop(thread::spawn(move || {
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(ChannelEndpoint::Client) else {
                    continue;
                };
                if let Err(error) = tx.try_send(Session::package_message(SessionEvent::SessionId(
                    runtime.state.session_id,
                ))) {
                    Logger::warning(format!("client channel session id send failed: {error}"));
                    continue;
                }
                Logger::log("client channel connected");
                *runtime
                    .state
                    .client_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *runtime
                    .state
                    .client_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                runtime.client_worker();
            }
        }));
    }

    fn spawn_host_worker(&self) {
        let runtime = self.clone();
        drop(thread::spawn(move || {
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionMessage>(ChannelEndpoint::Host) else {
                    continue;
                };
                if let Err(error) = tx.try_send(Session::package_message(SessionEvent::SessionId(
                    runtime.state.session_id,
                ))) {
                    Logger::warning(format!("host channel session id send failed: {error}"));
                    continue;
                }
                if let Err(error) = tx.try_send(Session::package_message(SessionEvent::Executor(
                    ExecutorEvent::ToolQuery,
                ))) {
                    Logger::warning(format!("host channel tool query send failed: {error}"));
                    continue;
                }
                Logger::log("host channel connected");
                *runtime
                    .state
                    .host_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *runtime
                    .state
                    .host_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::reset_context(&runtime.state);
                runtime.host_worker();
                Self::host_disconnect(&runtime.state);
            }
        }));
    }

    fn client_worker(&self) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build client event runtime: {error}"));
        rt.block_on(async {
            let Some(mut rx) = self
                .state
                .client_rx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            else {
                return;
            };
            while let Ok(Some(message)) = rx.recv().await {
                if let Err(error) = self.dispatch(message.event) {
                    match error {}
                }
            }
        });
    }

    fn host_worker(&self) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build host event runtime: {error}"));
        rt.block_on(async {
            let Some(mut rx) = self
                .state
                .host_rx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            else {
                return;
            };
            while let Ok(Some(message)) = rx.recv().await {
                if let Err(error) = self.dispatch(message.event) {
                    match error {}
                }
            }
        });
    }

    fn create_task(&self, request: TaskRequest) {
        let signature = request.signature.clone();
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
        if self
            .state
            .context
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .tools
            .is_empty()
        {
            let reason = "executor tools not registered".to_string();
            Logger::warning(format!("task {signature} rejected: {reason}"));
            self.send_client_event(SessionEvent::TaskUpdate(TaskStatus::Failed { reason }));
            return;
        }
        Logger::log(format!("task {signature} created"));
        self.send_client_event(SessionEvent::TaskUpdate(TaskStatus::Created));
        let task = Task::new(
            Arc::clone(&self.state.context),
            request,
            self.state.session_tx.clone(),
        );
        let context = self
            .state
            .context
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if context.tasks.with(&signature, |_| ()).is_some() {
            drop(context);
            Logger::warning(format!(
                "task {signature} create ignored: task already exists",
            ));
            return;
        }
        context.tasks.insert(signature, task.clone());
        drop(context);
        task.start();
    }

    fn dispatch_task(&self, signature: &TaskSignature, event: TaskEvent) {
        let task = self
            .state
            .context
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .tasks
            .with(signature, Clone::clone);
        let Some(task) = task else {
            Logger::warning(format!(
                "session could not dispatch event {event:?}: task {signature} not found",
            ));
            return;
        };
        task.dispatch(event);
    }

    fn register_executor_tools(&self, system: System, tools: Vec<ToolPreview>) {
        let tool_count = tools.len();
        let tool_names = tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let host_tx = self
            .state
            .host_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if host_tx.is_none() {
            Logger::warning("core session ignored executor tools: host disconnected");
            return;
        }
        drop(host_tx);
        *self
            .state
            .host_sys
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(system);
        let mut context = self
            .state
            .context
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        context.system = Some(system);
        context.tools = tools;
        drop(context);
        if tool_names.is_empty() {
            Logger::log("host registered 0 tools");
        } else {
            Logger::log(format!("host registered {tool_count} tools: {tool_names}"));
        }
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
            tasks: WorkQueue::new(),
            tools: Vec::new(),
        };
    }
}

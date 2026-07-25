use std::convert::Infallible;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

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

const HOST_ACCEPT_ERROR_BACKOFF: Duration = Duration::from_secs(1);

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

struct HostConnectionTermination {
    reason: &'static str,
    error: Option<String>,
    duration: Duration,
    messages_received: u64,
}

impl HostConnectionTermination {
    fn log(&self) {
        let message = format!(
            "host core connection terminated reason={} duration_ms={} \
             messages_received={} side=server",
            self.reason,
            self.duration.as_millis(),
            self.messages_received
        );
        if let Some(error) = &self.error {
            Logger::error_tagged(format!("{message} error={error}"), ["Host Connection"]);
        } else {
            Logger::warning_tagged(message, ["Host Connection"]);
        }
    }
}

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
                let (tx, rx) = match accept_channel::<SessionMessage>(
                    ChannelEndpoint::Host,
                ) {
                    Ok(channel) => channel,
                    Err(error) => {
                        Logger::error_tagged(
                            format!(
                                "host core connection accept failed \
                                 side=server action=continue_listening \
                                 backoff_ms={} error={error:?}",
                                HOST_ACCEPT_ERROR_BACKOFF.as_millis()
                            ),
                            ["Host Connection"],
                        );
                        thread::sleep(HOST_ACCEPT_ERROR_BACKOFF);
                        continue;
                    }
                };
                if let Err(error) = tx.try_send(Session::package_message(
                    SessionEvent::SessionId(runtime.state.session_id),
                )) {
                    Logger::error_tagged(
                        format!(
                            "host core connection initial send failed \
                             stage=session_id side=server \
                             action=continue_listening error={error}"
                        ),
                        ["Host Connection"],
                    );
                    continue;
                }
                if let Err(error) = tx.try_send(Session::package_message(
                    SessionEvent::Executor(ExecutorEvent::ToolQuery),
                )) {
                    Logger::error_tagged(
                        format!(
                            "host core connection initial send failed \
                             stage=tool_query side=server \
                             action=continue_listening error={error}"
                        ),
                        ["Host Connection"],
                    );
                    continue;
                }
                let connected_at = Instant::now();
                Logger::log_tagged(
                    "host core connection connected side=server",
                    ["Host Connection"],
                );
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
                let termination = runtime.host_worker(connected_at);
                termination.log();
                Self::host_disconnect(&runtime.state);
                Logger::log_tagged(
                    "host core connection relisten side=server",
                    ["Host Connection"],
                );
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

    fn host_worker(&self, connected_at: Instant) -> HostConnectionTermination {
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
                return HostConnectionTermination {
                    reason: "receiver_error",
                    error: Some(
                        "receiver unavailable after connection registration"
                            .to_owned(),
                    ),
                    duration: connected_at.elapsed(),
                    messages_received: 0,
                };
            };
            let mut messages_received = 0_u64;
            loop {
                match rx.recv().await {
                    Ok(Some(message)) => {
                        messages_received = messages_received.saturating_add(1);
                        if let Err(error) = self.dispatch(message.event) {
                            match error {}
                        }
                    }
                    Ok(None) => {
                        break HostConnectionTermination {
                            reason: "remote_closed",
                            error: None,
                            duration: connected_at.elapsed(),
                            messages_received,
                        };
                    }
                    Err(error) => {
                        break HostConnectionTermination {
                            reason: "receiver_error",
                            error: Some(error.to_string()),
                            duration: connected_at.elapsed(),
                            messages_received,
                        };
                    }
                }
            }
        })
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
                Logger::error_tagged(
                    format!(
                        "host core connection send failed phase=runtime \
                         side=server error={error}"
                    ),
                    ["Host Connection"],
                );
            }
        } else {
            Logger::warning_tagged(
                "host core connection send skipped reason=not_connected \
                 phase=runtime side=server",
                ["Host Connection"],
            );
        }
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
        *state
            .host_sys
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        Self::reset_context(state);
        Logger::log_tagged(
            "host core connection state cleanup completed side=server",
            ["Host Connection"],
        );
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

use std::net::SocketAddr;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::JoinHandle;

use crate::task::Task;
use marix_common::{Config, SessionEvent, TaskEvent, TaskId, TaskSignature, accept_channel};

use super::SessionContext;

pub struct Session {
    pub context: Arc<SessionContext>,
    client_worker: JoinHandle<()>,
    host_worker: JoinHandle<()>,
}

impl Session {
    pub fn new() -> Self {
        let context = Arc::new(SessionContext::new());
        let client_worker = Self::spawn_client_worker(&context);
        let host_worker = Self::spawn_host_worker(&context);
        Self {
            context,
            client_worker,
            host_worker,
        }
    }
}

// -- Private -- //

impl Session {
    fn parse_address(address: &str, label: &str) -> SocketAddr {
        address
            .parse()
            .unwrap_or_else(|error| panic!("invalid {label} bind address: {error}"))
    }

    fn spawn_client_worker(context: &Arc<SessionContext>) -> JoinHandle<()> {
        let context = Arc::clone(context);
        std::thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address = Self::parse_address(&config.agent.client_bind_address, "client");
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionEvent>(address) else {
                    continue;
                };
                *context
                    .client_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *context
                    .client_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::run_client_worker(Arc::clone(&context));
            }
        })
    }

    fn spawn_host_worker(context: &Arc<SessionContext>) -> JoinHandle<()> {
        let context = Arc::clone(context);
        std::thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address = Self::parse_address(&config.agent.host_bind_address, "host");
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionEvent>(address) else {
                    continue;
                };
                *context
                    .host_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(tx);
                *context
                    .host_rx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(rx);
                Self::run_host_worker(Arc::clone(&context));
            }
        })
    }

    fn run_client_worker(context: Arc<SessionContext>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build client event runtime: {error}"));
        runtime.block_on(async move {
            let Some(mut rx) = context
                .client_rx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            else {
                return;
            };
            while let Ok(Some(event)) = rx.recv().await {
                match event {
                    SessionEvent::Task(signature, TaskEvent::Create { request }) => {
                        Self::create_task(&context, signature, request);
                    }
                    event => Self::route_session_event(&context, event),
                }
            }
        });
    }

    fn run_host_worker(context: Arc<SessionContext>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build host event runtime: {error}"));
        runtime.block_on(async move {
            let Some(mut rx) = context
                .host_rx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            else {
                return;
            };
            while let Ok(Some(event)) = rx.recv().await {
                Self::route_session_event(&context, event);
            }
        });
    }

    fn create_task(context: &SessionContext, mut signature: TaskSignature, request: String) {
        signature.name = request;
        if context
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
            if let Some(sender) = context
                .client_tx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .as_mut()
            {
                let _ = sender.try_send(event);
            }
            return;
        }
        let task_id = signature.id.clone();
        let task = Task::new(
            signature,
            Arc::clone(&context.client_tx),
            Arc::clone(&context.host_tx),
        );
        context.tasks.insert(task_id, Arc::new(StdMutex::new(task)));
    }

    fn route_session_event(context: &SessionContext, event: SessionEvent) {
        match &event {
            SessionEvent::Task(signature, _) => {
                Self::route_task_event(context, &signature.id, event.clone());
            }
            SessionEvent::Execution(signature, _) => {
                Self::route_task_event(context, &signature.task_id, event.clone());
            }
        }
    }

    fn route_task_event(context: &SessionContext, task_id: &TaskId, event: SessionEvent) {
        let task = context.tasks.get(task_id.clone());
        let sender = task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .sender();
        let _ = sender.send(event);
    }
}

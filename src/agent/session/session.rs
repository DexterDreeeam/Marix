use std::net::SocketAddr;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::JoinHandle;

use crate::task::Task;
use marix_common::{Config, SessionEvent, TaskId, TaskSessionEvent, TaskSignature, accept_channel};

use super::SessionContext;

pub struct Session {
    pub context: Arc<SessionContext>,
    client_worker: Option<JoinHandle<()>>,
    host_worker: Option<JoinHandle<()>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            context: Arc::new(SessionContext::new()),
            client_worker: None,
            host_worker: None,
        }
    }

    pub fn run(&mut self) {
        if self.client_worker.is_none() {
            self.client_worker = Some(self.spawn_client_worker());
        }
        if self.host_worker.is_none() {
            self.host_worker = Some(self.spawn_host_worker());
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

    fn spawn_client_worker(&self) -> JoinHandle<()> {
        let context = Arc::clone(&self.context);
        std::thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address = Self::parse_address(&config.agent.client_bind_address, "client");
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionEvent>(address) else {
                    continue;
                };
                *context.client_tx.blocking_lock() = Some(tx);
                *context.client_rx.blocking_lock() = Some(rx);
                Self::handle_client_events(Arc::clone(&context));
            }
        })
    }

    fn spawn_host_worker(&self) -> JoinHandle<()> {
        let context = Arc::clone(&self.context);
        std::thread::spawn(move || {
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address = Self::parse_address(&config.agent.host_bind_address, "host");
            loop {
                let Ok((tx, rx)) = accept_channel::<SessionEvent>(address) else {
                    continue;
                };
                *context.host_tx.blocking_lock() = Some(tx);
                *context.host_rx.blocking_lock() = Some(rx);
                Self::handle_host_events(Arc::clone(&context));
            }
        })
    }

    fn handle_client_events(context: Arc<SessionContext>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build client event runtime: {error}"));
        runtime.block_on(async move {
            let Some(mut rx) = context.client_rx.lock().await.take() else {
                return;
            };
            while let Ok(Some(event)) = rx.recv().await {
                match event {
                    SessionEvent::Task(signature, TaskSessionEvent::Create { request }) => {
                        Self::create_task(&context, signature, request).await;
                    }
                    event => Self::route_session_event(&context, event),
                }
            }
        });
    }

    fn handle_host_events(context: Arc<SessionContext>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build host event runtime: {error}"));
        runtime.block_on(async move {
            let Some(mut rx) = context.host_rx.lock().await.take() else {
                return;
            };
            while let Ok(Some(event)) = rx.recv().await {
                Self::route_session_event(&context, event);
            }
        });
    }

    async fn create_task(context: &SessionContext, mut signature: TaskSignature, request: String) {
        signature.name = request;
        if context.host_tx.lock().await.is_none() {
            let event = SessionEvent::Task(
                signature,
                TaskSessionEvent::CreateFailed {
                    reason: "host not connected".to_string(),
                },
            );
            if let Some(sender) = context.client_tx.lock().await.as_mut() {
                let _ = sender.send(event).await;
            }
            return;
        }
        let task_id = signature.id.clone();
        let mut task = Task::new(
            signature,
            Arc::clone(&context.client_tx),
            Arc::clone(&context.host_tx),
        );
        task.run();
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

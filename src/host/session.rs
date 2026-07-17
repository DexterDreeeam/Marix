use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use marix_common::{ChannelEndpoint, Logger, NetReceiver, SharedNetSender, connect_channel};
use marix_protocol::{SessionEvent, SessionMessage};

use crate::executor::Executor;

static SOURCE_NAME: OnceLock<String> = OnceLock::new();

pub struct HostSession {
    worker: Option<JoinHandle<()>>,
    state: Arc<HostSessionState>,
}

impl HostSession {
    pub fn new(name: String) -> Self {
        let _ = SOURCE_NAME.set(name);
        Self {
            worker: None,
            state: Arc::new(HostSessionState::new()),
        }
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }

    pub fn close(&mut self) {
        self.state.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    pub fn run(&mut self) {
        if self.worker.is_some() {
            Logger::warning("host session run ignored: worker already running");
            return;
        }
        self.state.shutdown.store(false, Ordering::Relaxed);
        self.worker = Some(Self::spawn_worker(Arc::clone(&self.state)));
    }
}

// -- Private -- //

impl HostSession {
    fn spawn_worker(state: Arc<HostSessionState>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let mut executor = Executor::new(Arc::clone(&state.server_tx));
            let mut executor_started = false;
            while !state.shutdown.load(Ordering::Relaxed) {
                let Ok((net_tx, net_rx)) = connect_channel::<SessionMessage>(ChannelEndpoint::Host)
                else {
                    continue;
                };
                Logger::log("host connected to server core");
                *state
                    .server_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
                if !executor_started {
                    executor.start();
                    executor_started = true;
                }
                Self::worker(net_rx, &executor, &state.shutdown);
                *state
                    .server_tx
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = None;
            }
        })
    }

    fn worker(
        mut server_rx: NetReceiver<SessionMessage>,
        executor: &Executor,
        shutdown: &AtomicBool,
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build host event runtime: {error}"));
        runtime.block_on(async move {
            while let Ok(Some(message)) = server_rx.recv().await {
                match message.event {
                    SessionEvent::SessionId(id) => {
                        Logger::set_id(id);
                        Logger::log("host session id updated");
                    }
                    SessionEvent::Executor(event) => {
                        executor.dispatch(event);
                    }
                    event => {
                        Logger::warning(format!(
                            "host session received unsupported session event {event:?}"
                        ));
                    }
                }
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }
}

struct HostSessionState {
    shutdown: Arc<AtomicBool>,
    server_tx: SharedNetSender<SessionMessage>,
}

impl HostSessionState {
    fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            server_tx: SharedNetSender::new(std::sync::Mutex::new(None)),
        }
    }
}

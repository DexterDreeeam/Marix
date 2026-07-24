use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use marix_common::{
    ChannelEndpoint, Logger, NetReceiver, SharedNetSender, connect_channel_with_timeout,
};
use marix_protocol::{SessionEvent, SessionMessage};

use crate::executor::Executor;

/// Host's connect attempt to the server core is single-shot and bounded
/// by this timeout; unlike Client, Host never retries a failed or
/// dropped connection (see [`HostSession::spawn_worker`]).
const HOST_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

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
            let (net_tx, net_rx) = match connect_channel_with_timeout::<SessionMessage>(
                ChannelEndpoint::Host,
                HOST_CONNECT_TIMEOUT,
            ) {
                Ok(channel) => channel,
                Err(error) => {
                    // A spawned thread panic would not stop the process
                    // (main just parks forever), so this must exit the
                    // OS process directly for a deployment script's
                    // "process still running" check to stay meaningful.
                    Logger::error(format!(
                        "host failed to connect to server core within {}s: {error:?}",
                        HOST_CONNECT_TIMEOUT.as_secs()
                    ));
                    std::process::exit(1);
                }
            };
            Logger::log("host connected to server core");
            *state
                .server_tx
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
            executor.start();
            Self::worker(net_rx, &executor, &state.shutdown);
            Logger::error("host lost connection to server core");
            std::process::exit(1);
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

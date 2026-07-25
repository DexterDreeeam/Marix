use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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

enum HostConnectionTerminationReason {
    LocalShutdown,
    RemoteClosed,
    ReceiverError(String),
}

struct HostConnectionTermination {
    reason: HostConnectionTerminationReason,
    duration: Duration,
    messages_received: u64,
}

impl HostConnectionTermination {
    fn new(
        reason: HostConnectionTerminationReason,
        connected_at: Instant,
        messages_received: u64,
    ) -> Self {
        Self {
            reason,
            duration: connected_at.elapsed(),
            messages_received,
        }
    }

    fn log(&self) {
        let duration_ms = self.duration.as_millis();
        let messages_received = self.messages_received;
        match &self.reason {
            HostConnectionTerminationReason::LocalShutdown => {
                Logger::log_tagged(
                    format!(
                        "host core connection terminated reason=local_shutdown \
                         duration_ms={duration_ms} messages_received={messages_received} \
                         side=host action=process_exit exit_code=1"
                    ),
                    ["Host Connection"],
                );
            }
            HostConnectionTerminationReason::RemoteClosed => {
                Logger::warning_tagged(
                    format!(
                        "host core connection terminated reason=remote_closed \
                         duration_ms={duration_ms} messages_received={messages_received} \
                         side=host action=process_exit exit_code=1"
                    ),
                    ["Host Connection"],
                );
            }
            HostConnectionTerminationReason::ReceiverError(error) => {
                Logger::error_tagged(
                    format!(
                        "host core connection terminated reason=receiver_error \
                         duration_ms={duration_ms} messages_received={messages_received} \
                         side=host action=process_exit exit_code=1 error={error}"
                    ),
                    ["Host Connection"],
                );
            }
        }
    }
}

impl HostSession {
    fn spawn_worker(state: Arc<HostSessionState>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let mut executor = Executor::new(Arc::clone(&state.server_tx));
            Logger::log_tagged(
                format!(
                    "host core connection attempt timeout_ms={} side=host policy=single_attempt",
                    HOST_CONNECT_TIMEOUT.as_millis()
                ),
                ["Host Connection"],
            );
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
                    Logger::error_tagged(
                        format!(
                            "host core connection attempt failed reason=connect_error \
                             timeout_ms={} side=host action=process_exit exit_code=1 \
                             error={error:?}",
                            HOST_CONNECT_TIMEOUT.as_millis()
                        ),
                        ["Host Connection"],
                    );
                    std::process::exit(1);
                }
            };
            let connected_at = Instant::now();
            Logger::log_tagged(
                "host core connection connected side=host",
                ["Host Connection"],
            );
            *state
                .server_tx
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
            executor.start();
            let termination =
                Self::worker(net_rx, &executor, &state.shutdown, connected_at);
            termination.log();
            std::process::exit(1);
        })
    }

    fn worker(
        mut server_rx: NetReceiver<SessionMessage>,
        executor: &Executor,
        shutdown: &AtomicBool,
        connected_at: Instant,
    ) -> HostConnectionTermination {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build host event runtime: {error}"));
        runtime.block_on(async move {
            let mut messages_received = 0_u64;
            loop {
                if shutdown.load(Ordering::Relaxed) {
                    break HostConnectionTermination::new(
                        HostConnectionTerminationReason::LocalShutdown,
                        connected_at,
                        messages_received,
                    );
                }
                match server_rx.recv().await {
                    Ok(Some(message)) => {
                        messages_received =
                            messages_received.saturating_add(1);
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
                    }
                    Ok(None) => {
                        break HostConnectionTermination::new(
                            HostConnectionTerminationReason::RemoteClosed,
                            connected_at,
                            messages_received,
                        );
                    }
                    Err(error) => {
                        break HostConnectionTermination::new(
                            HostConnectionTerminationReason::ReceiverError(
                                error.to_string(),
                            ),
                            connected_at,
                            messages_received,
                        );
                    }
                }
            }
        })
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

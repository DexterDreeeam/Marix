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
    shutdown: Arc<AtomicBool>,
}

impl HostSession {
    pub fn new(name: String) -> Self {
        let _ = SOURCE_NAME.set(name);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = Self::spawn_worker(Arc::clone(&shutdown));
        Self {
            worker: Some(worker),
            shutdown,
        }
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }

    pub fn close(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

// -- Private -- //

impl HostSession {
    fn spawn_worker(shutdown: Arc<AtomicBool>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let server_tx: SharedNetSender<SessionMessage> =
                SharedNetSender::new(std::sync::Mutex::new(None));
            let executor = Executor::new(Arc::clone(&server_tx));
            while !shutdown.load(Ordering::Relaxed) {
                let Ok((net_tx, net_rx)) = connect_channel::<SessionMessage>(ChannelEndpoint::Host)
                else {
                    continue;
                };
                let _ = Logger::log("host connected to server core");
                *server_tx.lock().unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
                Self::worker(net_rx, &executor, &shutdown);
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
                    SessionEvent::Executor(event) => {
                        if executor.sender().send(event).is_err() {
                            let _ = Logger::warning(
                                "host session could not send executor event: \
                                 executor worker stopped",
                            );
                        }
                    }
                    event => {
                        let _ = Logger::warning(format!(
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

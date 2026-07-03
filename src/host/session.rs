use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use marix_common::{Config, NetReceiver, SessionEvent, SharedNetSender, connect_channel};

use crate::executor::Executor;

pub struct HostSession {
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
}

impl HostSession {
    pub fn new() -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = Self::spawn_worker(Arc::clone(&shutdown));
        Self {
            worker: Some(worker),
            shutdown,
        }
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
            let config =
                Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
            let address: SocketAddr = config
                .agent
                .host_bind_address
                .parse()
                .unwrap_or_else(|error| panic!("invalid host bind address: {error}"));
            let agent_tx: SharedNetSender<SessionEvent> =
                SharedNetSender::new(std::sync::Mutex::new(None));
            let mut executor = Executor::new(Arc::clone(&agent_tx));
            while !shutdown.load(Ordering::Relaxed) {
                let Ok((net_tx, net_rx)) = connect_channel::<SessionEvent>(address) else {
                    continue;
                };
                *agent_tx.lock().unwrap_or_else(|error| error.into_inner()) = Some(net_tx);
                Self::run_worker(net_rx, &mut executor, &shutdown);
            }
        })
    }

    fn run_worker(
        mut agent_rx: NetReceiver<SessionEvent>,
        executor: &mut Executor,
        shutdown: &AtomicBool,
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build host event runtime: {error}"));
        runtime.block_on(async move {
            while let Ok(Some(event)) = agent_rx.recv().await {
                executor.route_session_event(event);
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }
}

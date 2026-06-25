use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicResponseSignal {
    Changed,
    Finished,
    Failed(String),
    TimedOut,
}

struct DynamicResponseInner<T> {
    state: RwLock<T>,
    cancelled: AtomicBool,
    completed: AtomicBool,
    failed: Mutex<Option<String>>,
    version: AtomicU64,
    signal_lock: Mutex<()>,
    signal: Condvar,
}

impl<T> DynamicResponseInner<T> {
    fn snapshot_terminal(&self) -> Option<DynamicResponseSignal> {
        if let Some(reason) = self.failed.lock().expect("failed lock").as_ref() {
            return Some(DynamicResponseSignal::Failed(reason.clone()));
        }
        if self.cancelled.load(Ordering::Acquire) || self.completed.load(Ordering::Acquire) {
            return Some(DynamicResponseSignal::Finished);
        }
        None
    }

    fn try_cancel(&self) -> bool {
        let _guard = self.signal_lock.lock().expect("signal lock");
        if self.snapshot_terminal().is_some() {
            return false;
        }
        self.cancelled.store(true, Ordering::Release);
        true
    }

    fn try_complete(&self) -> bool {
        let _guard = self.signal_lock.lock().expect("signal lock");
        if self.snapshot_terminal().is_some() {
            return false;
        }
        self.completed.store(true, Ordering::Release);
        true
    }

    fn try_fail(&self, reason: String) -> bool {
        let _guard = self.signal_lock.lock().expect("signal lock");
        if self.snapshot_terminal().is_some() {
            return false;
        }
        *self.failed.lock().expect("failed lock") = Some(reason);
        true
    }

    fn notify(&self) {
        let _guard = self.signal_lock.lock().expect("signal lock");
        self.signal.notify_all();
    }

    fn wait(&self, timeout: Option<Duration>, observed_version: &mut u64) -> DynamicResponseSignal {
        if let Some(signal) = self.snapshot_terminal() {
            return signal;
        }
        if self.has_changed(observed_version) {
            return DynamicResponseSignal::Changed;
        }
        let guard = self.signal_lock.lock().expect("signal lock");
        if let Some(signal) = self.snapshot_terminal() {
            return signal;
        }
        if self.has_changed(observed_version) {
            return DynamicResponseSignal::Changed;
        }
        let (_guard, timed_out) = match timeout {
            None => (self.signal.wait(guard).expect("signal wait"), false),
            Some(duration) => {
                let (guard, result) = self
                    .signal
                    .wait_timeout(guard, duration)
                    .expect("signal wait timeout");
                (guard, result.timed_out())
            }
        };
        if let Some(signal) = self.snapshot_terminal() {
            return signal;
        }
        if self.has_changed(observed_version) {
            return DynamicResponseSignal::Changed;
        }
        if timed_out {
            DynamicResponseSignal::TimedOut
        } else {
            DynamicResponseSignal::Changed
        }
    }

    fn wait_for_cancel(&self, timeout: Option<Duration>) -> bool {
        let deadline = timeout.map(|duration| Instant::now() + duration);
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return true;
            }
            if self.completed.load(Ordering::Acquire)
                || self.failed.lock().expect("failed lock").is_some()
            {
                return false;
            }
            let guard = self.signal_lock.lock().expect("signal lock");
            if self.cancelled.load(Ordering::Acquire) {
                return true;
            }
            if self.completed.load(Ordering::Acquire)
                || self.failed.lock().expect("failed lock").is_some()
            {
                return false;
            }
            let timed_out = match deadline {
                None => {
                    let _guard = self.signal.wait(guard).expect("signal wait");
                    false
                }
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return self.cancelled.load(Ordering::Acquire);
                    }
                    let (_guard, result) = self
                        .signal
                        .wait_timeout(guard, deadline - now)
                        .expect("signal wait timeout");
                    result.timed_out()
                }
            };
            if timed_out {
                return self.cancelled.load(Ordering::Acquire);
            }
        }
    }

    fn update(&self, update: impl FnOnce(&mut T)) {
        let mut state = self.state.write().expect("state write");
        update(&mut state);
        drop(state);
        self.version.fetch_add(1, Ordering::Release);
        self.notify();
    }

    fn has_changed(&self, observed_version: &mut u64) -> bool {
        let current_version = self.version.load(Ordering::Acquire);
        if current_version == *observed_version {
            return false;
        }
        *observed_version = current_version;
        true
    }
}

pub struct DynamicResponse<T: Send + Sync + 'static> {
    inner: Arc<DynamicResponseInner<T>>,
    observed_version: Mutex<u64>,
}

pub struct DynamicResponseProducer<T: Send + Sync + 'static> {
    inner: Arc<DynamicResponseInner<T>>,
}

impl<T: Send + Sync + 'static> DynamicResponse<T> {
    pub fn new(initial: T) -> (Self, DynamicResponseProducer<T>) {
        let inner = Arc::new(DynamicResponseInner {
            state: RwLock::new(initial),
            cancelled: AtomicBool::new(false),
            completed: AtomicBool::new(false),
            failed: Mutex::new(None),
            version: AtomicU64::new(0),
            signal_lock: Mutex::new(()),
            signal: Condvar::new(),
        });
        (
            Self {
                inner: inner.clone(),
                observed_version: Mutex::new(0),
            },
            DynamicResponseProducer { inner },
        )
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner.state.read().expect("state read").clone()
    }

    pub fn cancel(&self) {
        if self.inner.try_cancel() {
            self.inner.notify();
        }
    }

    pub fn wait(&self, timeout: Option<Duration>) -> DynamicResponseSignal {
        let mut observed_version = self.observed_version.lock().expect("observed version lock");
        self.inner.wait(timeout, &mut observed_version)
    }
}

impl<T: Send + Sync + 'static> Drop for DynamicResponse<T> {
    fn drop(&mut self) {
        if self.inner.try_cancel() {
            self.inner.notify();
        }
    }
}

impl<T: Send + Sync + 'static> DynamicResponseProducer<T> {
    pub fn update(&self, update: impl FnOnce(&mut T)) {
        self.inner.update(update);
    }

    pub fn complete(&self) {
        if self.inner.try_complete() {
            self.inner.notify();
        }
    }

    pub fn fail(&self, reason: impl Into<String>) {
        if self.inner.try_fail(reason.into()) {
            self.inner.notify();
        }
    }

    pub fn wait_for_cancel(&self, timeout: Option<Duration>) -> bool {
        self.inner.wait_for_cancel(timeout)
    }

    pub fn spawn<Worker>(self, worker: Worker)
    where
        Worker: FnOnce(&Self) + Send + 'static,
    {
        std::thread::spawn(move || {
            let producer = self;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                worker(&producer);
            }));
            if let Err(payload) = result {
                producer.fail(format!(
                    "dynamic response worker panicked: {}",
                    panic_payload_to_string(payload)
                ));
            }
        });
    }
}

impl<T: Send + Sync + 'static> Drop for DynamicResponseProducer<T> {
    fn drop(&mut self) {
        if self.inner.try_complete() {
            self.inner.notify();
        }
    }
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_owned();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "non-string panic payload".to_owned()
}

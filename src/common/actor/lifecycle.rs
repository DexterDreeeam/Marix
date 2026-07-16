use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::external::*;
use crate::structure::{AsyncReceiver, AsyncSender, build_async_channel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ActorStatus {
    Created = 0,
    Running = 1,
    Complete = 2,
}

impl ActorStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Complete)
    }
}

pub struct Lifecycle<Event, Result> {
    status: AtomicU8,
    result: Mutex<Option<Result>>,
    event_tx: AsyncSender<Event>,
    event_rx: Mutex<Option<AsyncReceiver<Event>>>,
    close_tx: AsyncSender<()>,
    close_rx: Mutex<Option<AsyncReceiver<()>>>,
}

impl<Event, Result> Lifecycle<Event, Result>
where
    Result: Clone,
{
    pub fn new() -> Self {
        let (event_tx, event_rx) = build_async_channel();
        let (close_tx, close_rx) = build_async_channel();
        Self {
            status: AtomicU8::new(ActorStatus::Created as u8),
            result: Mutex::new(None),
            event_tx,
            event_rx: Mutex::new(Some(event_rx)),
            close_tx,
            close_rx: Mutex::new(Some(close_rx)),
        }
    }

    pub fn status(&self) -> ActorStatus {
        match self.status.load(Ordering::Acquire) {
            value if value == ActorStatus::Created as u8 => ActorStatus::Created,
            value if value == ActorStatus::Running as u8 => ActorStatus::Running,
            value if value == ActorStatus::Complete as u8 => ActorStatus::Complete,
            _ => unreachable!("lifecycle stored an invalid actor status"),
        }
    }

    pub fn result(&self) -> Option<Result> {
        self.lock_result().clone()
    }
}

// -- Private -- //

impl<Event, Result> Lifecycle<Event, Result>
where
    Result: Clone,
{
    pub(super) fn begin(&self) -> Option<(AsyncReceiver<Event>, AsyncReceiver<()>)> {
        if self
            .status
            .compare_exchange(
                ActorStatus::Created as u8,
                ActorStatus::Running as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return None;
        }
        let event_rx = self
            .event_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()?;
        let close_rx = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()?;
        Some((event_rx, close_rx))
    }

    pub(super) fn dispatch(&self, event: Event) -> bool {
        self.event_tx.send(event).is_ok()
    }

    pub(super) fn finish(&self, result: Result) -> bool {
        let mut stored = self.lock_result();
        if self.status().is_terminal() {
            return false;
        }
        *stored = Some(result);
        self.status
            .store(ActorStatus::Complete as u8, Ordering::Release);
        true
    }

    pub(super) fn close(&self) -> bool {
        self.close_tx.send(()).is_ok()
    }

    fn lock_result(&self) -> MutexGuard<'_, Option<Result>> {
        self.result
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

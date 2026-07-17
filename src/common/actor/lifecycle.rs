use std::sync::{Mutex, MutexGuard};

use crate::external::*;
use crate::structure::{AsyncReceiver, AsyncSender, build_async_channel};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorStatus<Result> {
    Created,
    Running,
    Complete(Result),
}

pub struct Lifecycle<Event, Result> {
    state: Mutex<ActorStatus<Result>>,
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
            state: Mutex::new(ActorStatus::Created),
            event_tx,
            event_rx: Mutex::new(Some(event_rx)),
            close_tx,
            close_rx: Mutex::new(Some(close_rx)),
        }
    }

    pub fn status(&self) -> ActorStatus<Result> {
        self.lock_state().clone()
    }

    pub fn result(&self) -> Option<Result> {
        let state = self.lock_state();
        match &*state {
            ActorStatus::Complete(result) => Some(result.clone()),
            ActorStatus::Created | ActorStatus::Running => None,
        }
    }
}

// -- Private -- //

impl<Event, Result> Lifecycle<Event, Result>
where
    Result: Clone,
{
    pub(super) fn begin(&self) -> Option<(AsyncReceiver<Event>, AsyncReceiver<()>)> {
        {
            let mut state = self.lock_state();
            if !matches!(&*state, ActorStatus::Created) {
                return None;
            }
            *state = ActorStatus::Running;
        }

        let mut event_rx = self
            .event_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut close_rx = self
            .close_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if event_rx.is_none() || close_rx.is_none() {
            return None;
        }

        Some((event_rx.take()?, close_rx.take()?))
    }

    pub(super) fn dispatch(&self, event: Event) -> bool {
        self.event_tx.send(event).is_ok()
    }

    pub(super) fn finish(&self, result: Result) -> bool {
        let mut state = self.lock_state();
        if matches!(&*state, ActorStatus::Complete(_)) {
            return false;
        }
        *state = ActorStatus::Complete(result);
        true
    }

    pub(super) fn close(&self) -> bool {
        self.close_tx.send(()).is_ok()
    }

    fn lock_state(&self) -> MutexGuard<'_, ActorStatus<Result>> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

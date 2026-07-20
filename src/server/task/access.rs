use std::fmt::{Debug, Display};
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::Instant;

use marix_common::external::*;
use marix_common::{Actor, ResultOf, Sender, SignatureOf, WorkQueue};
use marix_protocol::{
    IntentSignature, InvocationSignature, RelaySignature, SessionEvent, StepSignature,
    TaskSignature,
};

use crate::intent::Intent;
use crate::invocation::Invocation;
use crate::relay::Relay;
use crate::session::SessionContext;
use crate::step::Step;

const COMPLETION_TIME_EXCEEDED: &str = "maximum completion time exceeded";
const RELAY_COUNT_EXCEEDED: &str = "maximum relay count exceeded";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskGate {
    Step,
    Relay,
}

pub(crate) trait StoredActor: Actor + Clone {
    fn get(access: &TaskAccess, signature: &SignatureOf<Self>) -> Option<Self>;
    fn insert(access: &TaskAccess, actor: Self) -> bool;
}

pub(crate) trait StoredSignature: Display + Clone + Debug + Send + Sync + 'static {
    type Actor: StoredActor<Signature = Self>;
}

pub struct TaskAccess {
    pub session_context: Weak<StdMutex<SessionContext>>,
    pub session_tx: Sender<SessionEvent>,
    pub signature: TaskSignature,
    pub user_request: String,
    pub rt: Arc<tokio::Runtime>,
    deadline: Option<Instant>,
    left_relay: Option<AtomicUsize>,
    intents: Arc<WorkQueue<IntentSignature, Intent>>,
    steps: Arc<WorkQueue<StepSignature, Step>>,
    invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
    relays: Arc<WorkQueue<RelaySignature, Relay>>,
}

// -- Private -- //

impl TaskAccess {
    pub(crate) fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        session_tx: Sender<SessionEvent>,
        signature: TaskSignature,
        user_request: String,
        deadline: Option<Instant>,
        left_relay: Option<usize>,
        intents: Arc<WorkQueue<IntentSignature, Intent>>,
        steps: Arc<WorkQueue<StepSignature, Step>>,
        invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
        relays: Arc<WorkQueue<RelaySignature, Relay>>,
    ) -> Arc<Self> {
        let rt = tokio::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build task runtime: {error}"));
        Arc::new(Self {
            session_context: Arc::downgrade(&session_context),
            session_tx,
            signature,
            user_request,
            rt: Arc::new(rt),
            deadline,
            left_relay: left_relay.map(AtomicUsize::new),
            intents,
            steps,
            invocations,
            relays,
        })
    }
}

impl TaskAccess {
    pub(crate) fn session_context(&self) -> Result<Arc<StdMutex<SessionContext>>, String> {
        self.session_context.upgrade().ok_or_else(|| {
            format!(
                "task {} session context is no longer available",
                &self.signature,
            )
        })
    }

    pub(crate) fn get_result<S>(&self, signature: &S) -> Option<ResultOf<S::Actor>>
    where
        S: StoredSignature,
    {
        S::Actor::get(self, signature).and_then(|actor| actor.result())
    }

    pub(crate) fn insert<A: StoredActor>(&self, actor: A) -> bool {
        A::insert(self, actor)
    }

    pub(crate) fn completion_deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub(crate) fn gate(&self, gate: TaskGate) -> Result<(), String> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(COMPLETION_TIME_EXCEEDED.to_owned());
        }
        if matches!(gate, TaskGate::Relay) {
            self.reserve_relay()?;
        }
        Ok(())
    }
}

impl TaskAccess {
    fn reserve_relay(&self) -> Result<(), String> {
        let Some(left_relay) = &self.left_relay else {
            return Ok(());
        };
        left_relay
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |left| {
                left.checked_sub(1)
            })
            .map(|_| ())
            .map_err(|_| RELAY_COUNT_EXCEEDED.to_owned())
    }
}

impl StoredSignature for IntentSignature {
    type Actor = Intent;
}

impl StoredSignature for StepSignature {
    type Actor = Step;
}

impl StoredSignature for InvocationSignature {
    type Actor = Invocation;
}

impl StoredSignature for RelaySignature {
    type Actor = Relay;
}

impl StoredActor for Intent {
    fn get(access: &TaskAccess, signature: &SignatureOf<Self>) -> Option<Self> {
        access.intents.with(signature, Clone::clone)
    }

    fn insert(access: &TaskAccess, actor: Self) -> bool {
        let signature = actor.signature().clone();
        if access.intents.with(&signature, |_| ()).is_some() {
            return false;
        }
        access.intents.insert(signature, actor);
        true
    }
}

impl StoredActor for Step {
    fn get(access: &TaskAccess, signature: &SignatureOf<Self>) -> Option<Self> {
        access.steps.with(signature, Clone::clone)
    }

    fn insert(access: &TaskAccess, actor: Self) -> bool {
        let signature = actor.signature().clone();
        if access.steps.with(&signature, |_| ()).is_some() {
            return false;
        }
        access.steps.insert(signature, actor);
        true
    }
}

impl StoredActor for Invocation {
    fn get(access: &TaskAccess, signature: &SignatureOf<Self>) -> Option<Self> {
        access.invocations.with(signature, Clone::clone)
    }

    fn insert(access: &TaskAccess, actor: Self) -> bool {
        let signature = actor.signature().clone();
        if access.invocations.with(&signature, |_| ()).is_some() {
            return false;
        }
        access.invocations.insert(signature, actor);
        true
    }
}

impl StoredActor for Relay {
    fn get(access: &TaskAccess, signature: &SignatureOf<Self>) -> Option<Self> {
        access.relays.with(signature, Clone::clone)
    }

    fn insert(access: &TaskAccess, actor: Self) -> bool {
        let signature = actor.signature().clone();
        if access.relays.with(&signature, |_| ()).is_some() {
            return false;
        }
        access.relays.insert(signature, actor);
        true
    }
}

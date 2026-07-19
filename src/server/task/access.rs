use std::fmt::{Debug, Display};
use std::sync::Mutex as StdMutex;
use std::sync::{Arc, Weak};

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

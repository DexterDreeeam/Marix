use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{Sender, WorkQueue};
use marix_protocol::{
    IntentResult, IntentSignature, InvocationResult, InvocationSignature, PlanSignature,
    RelaySignature, SessionEvent, StepSignature, TaskSignature,
};

use crate::intent::Intent;
use crate::invocation::Invocation;
use crate::plan::Plan;
use crate::relay::Relay;
use crate::session::SessionContext;
use crate::step::Step;

pub struct TaskAccess {
    pub session_context: Arc<StdMutex<SessionContext>>,
    pub session_tx: Sender<SessionEvent>,
    pub signature: TaskSignature,
    pub user_request: String,
    pub rt: Arc<tokio::Runtime>,
    intents: Arc<WorkQueue<IntentSignature, Intent>>,
    plans: Arc<WorkQueue<PlanSignature, Plan>>,
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
        plans: Arc<WorkQueue<PlanSignature, Plan>>,
        steps: Arc<WorkQueue<StepSignature, Step>>,
        invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
        relays: Arc<WorkQueue<RelaySignature, Relay>>,
    ) -> Arc<Self> {
        let rt = tokio::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| panic!("failed to build task runtime: {error}"));
        Arc::new(Self {
            session_context,
            session_tx,
            signature,
            user_request,
            rt: Arc::new(rt),
            intents,
            plans,
            steps,
            invocations,
            relays,
        })
    }
}

impl TaskAccess {
    pub(crate) fn get_result(&self, signature: &IntentSignature) -> Option<IntentResult> {
        self.intents.with(signature, Intent::result).flatten()
    }

    pub(crate) fn get_intent_content(&self, signature: &IntentSignature) -> Option<String> {
        self.intents
            .with(signature, |intent| intent.state.content.clone())
    }

    pub(crate) fn get_invocation_result(
        &self,
        signature: &InvocationSignature,
    ) -> Option<InvocationResult> {
        self.invocations
            .with(signature, Invocation::result)
            .flatten()
    }

    pub(crate) fn insert_intent(&self, intent: Intent) -> bool {
        let signature = intent.state.signature.clone();
        if self.intents.with(&signature, |_| ()).is_some() {
            return false;
        }
        self.intents.insert(signature, intent);
        true
    }

    pub(crate) fn insert_plan(&self, plan: Plan) -> bool {
        let signature = plan.state.signature.clone();
        if self.plans.with(&signature, |_| ()).is_some() {
            return false;
        }
        self.plans.insert(signature, plan);
        true
    }

    pub(crate) fn insert_step(&self, step: Step) -> bool {
        let signature = step.state.signature.clone();
        if self.steps.with(&signature, |_| ()).is_some() {
            return false;
        }
        self.steps.insert(signature, step);
        true
    }

    pub(crate) fn insert_invocation(&self, invocation: Invocation) -> bool {
        let signature = invocation.state.signature.clone();
        if self.invocations.with(&signature, |_| ()).is_some() {
            return false;
        }
        self.invocations.insert(signature, invocation);
        true
    }

    pub(crate) fn insert_relay(&self, relay: Relay) -> bool {
        let signature = relay.state.signature.clone();
        if self.relays.with(&signature, |_| ()).is_some() {
            return false;
        }
        self.relays.insert(signature, relay);
        true
    }
}

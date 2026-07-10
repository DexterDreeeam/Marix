use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{Logger, Sender, WorkQueue};
use marix_protocol::{
    InvocationSignature, PlanError, PlanSignature, RelaySignature, SessionEvent, StepSignature,
    TaskSignature,
};

use crate::invocation::Invocation;
use crate::plan::Plan;
use crate::relay::Relay;
use crate::session::SessionContext;
use crate::step::Step;

#[derive(Clone)]
pub struct TaskAccess {
    pub session_context: Arc<StdMutex<SessionContext>>,
    pub session_tx: Sender<SessionEvent>,
    pub signature: TaskSignature,
    pub user_request: String,
    pub rt: Arc<tokio::Runtime>,
    plans: Arc<WorkQueue<PlanSignature, Plan>>,
    invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
    relays: Arc<WorkQueue<RelaySignature, Relay>>,
    steps: Arc<WorkQueue<StepSignature, Step>>,
}

impl TaskAccess {
    pub(super) fn new(
        session_context: Arc<StdMutex<SessionContext>>,
        session_tx: Sender<SessionEvent>,
        signature: TaskSignature,
        user_request: String,
        plans: Arc<WorkQueue<PlanSignature, Plan>>,
        invocations: Arc<WorkQueue<InvocationSignature, Invocation>>,
        relays: Arc<WorkQueue<RelaySignature, Relay>>,
        steps: Arc<WorkQueue<StepSignature, Step>>,
    ) -> Self {
        let rt = tokio::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|error| {
                panic!("failed to build task runtime: {error}")
            });
        Self {
            session_context,
            session_tx,
            signature,
            user_request,
            rt: Arc::new(rt),
            plans,
            invocations,
            relays,
            steps,
        }
    }

    pub(crate) fn insert_invocation(&self, invocation: Invocation) -> bool {
        let signature = invocation.signature().clone();
        if self.invocations.with(&signature, |_| ()).is_some() {
            Logger::warning(format!(
                "invocation {} create ignored: invocation already exists",
                &signature,
            ));
            return false;
        }
        self.invocations.insert(signature, invocation);
        true
    }

    pub(crate) fn insert_relay(&self, relay: Relay) -> bool {
        let signature = relay.signature().clone();
        if self.relays.with(&signature, |_| ()).is_some() {
            Logger::warning(format!(
                "relay {} create ignored: relay already exists",
                &signature,
            ));
            return false;
        }
        self.relays.insert(signature, relay);
        true
    }

    pub(crate) fn insert_plan(&self, plan: Plan) -> Result<(), PlanError> {
        let signature = plan.state.signature.clone();
        if self.plans.with(&signature, |_| ()).is_some() {
            return Err(PlanError::DuplicatePlan);
        }
        self.plans.insert(signature, plan);
        Ok(())
    }
}

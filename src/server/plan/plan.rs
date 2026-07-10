use std::fmt;
use std::sync::Arc;

use marix_common::external::*;
use marix_common::Logger;
use marix_protocol::{
    Actor, PlanDraft, PlanError, PlanEvent, PlanSignature, RuntimeAsync,
};

use super::runtime::PlanRuntime;
use super::state::PlanState;
use crate::step::Step;
use crate::task::TaskAccess;

pub struct Plan {
    pub(crate) state: Arc<PlanState>,
}

impl Clone for Plan {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl Plan {
    pub(crate) fn new(
        access: TaskAccess,
        signature: PlanSignature,
        description: String,
        background: String,
        call: Vec<Step>,
        model: Step,
        future: Vec<Step>,
        expected_result: String,
    ) -> Self {
        Self {
            state: Arc::new(PlanState::new(
                access,
                signature,
                description,
                background,
                call,
                model,
                future,
                expected_result,
            )),
        }
    }

    pub(crate) fn from_draft(
        access: TaskAccess,
        signature: PlanSignature,
        draft: PlanDraft,
    ) -> Result<Self, PlanError> {
        let call = draft
            .call
            .into_iter()
            .map(|draft| Step::from_draft(access.clone(), &signature, draft))
            .collect::<Result<Vec<_>, _>>()?;
        let model = Step::from_draft(
            access.clone(),
            &signature,
            draft.model,
        )?;
        let future = draft
            .future
            .into_iter()
            .map(|draft| Step::from_draft(access.clone(), &signature, draft))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::new(
            access,
            signature,
            draft.description,
            draft.background,
            call,
            model,
            future,
            draft.expected_result,
        ))
    }
}

impl Actor<Plan, PlanEvent> for Plan {
    fn start(&mut self) {
        let runtime = PlanRuntime::new(Arc::clone(&self.state));
        drop(self.state.access.rt.spawn(async move {
            runtime.run().await;
        }));
    }

    fn dispatch(&self, event: PlanEvent) {
        if let Err(error) = self.state.plan_tx.send(event) {
            let event = error.0;
            Logger::warning(format!(
                "plan {} event {event:?} dispatch failed: worker stopped (task {})",
                &self.state.signature,
                &self.state.signature.task,
            ));
        }
    }
}

impl fmt::Debug for Plan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let call = self
            .state
            .call
            .iter()
            .map(|step| step.signature())
            .collect::<Vec<_>>();
        let future = self
            .state
            .future
            .iter()
            .map(|step| step.signature())
            .collect::<Vec<_>>();
        let model = self.state.model.signature();
        formatter
            .debug_struct("Plan")
            .field("signature", &self.state.signature)
            .field("description", &self.state.description)
            .field("background", &self.state.background)
            .field("call", &call)
            .field("model", &model)
            .field("future", &future)
            .field("expected_result", &self.state.expected_result)
            .finish()
    }
}

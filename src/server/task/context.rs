use marix_common::Actor;
use marix_protocol::{ContextChain, IntentContext, IntentSignature};

use super::TaskAccess;
use super::access::StoredActor;
use crate::intent::Intent;

impl TaskAccess {
    pub(crate) fn index_of(&self, signature: &IntentSignature) -> Result<Option<usize>, String> {
        let Some(parent) = signature.parent.as_deref() else {
            return Ok(None);
        };
        let parent_intent = <Intent as StoredActor>::get(self, parent).ok_or_else(|| {
            format!(
                "cannot index intent {signature}: parent intent \
                     {parent} was not found",
            )
        })?;
        let plan = parent_intent
            .runtime
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let plan = plan.as_ref().ok_or_else(|| {
            format!(
                "cannot index intent {signature}: parent intent {parent} \
                 has no active plan",
            )
        })?;
        plan.subintents
            .iter()
            .position(|candidate| candidate == signature)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "intent {signature} is not in parent intent \
                     {parent}'s current subintents; it may be stale \
                     after its parent plan ended or changed",
                )
            })
    }

    pub(crate) fn get_context_chain(
        &self,
        signature: &IntentSignature,
    ) -> Result<ContextChain, String> {
        let mut intents = Vec::new();
        self.collect_context(signature, &mut intents)?;
        Ok(ContextChain { intents })
    }

    pub(crate) fn get_intent_context(
        &self,
        signature: &IntentSignature,
    ) -> Result<IntentContext, String> {
        self.index_of(signature)?;
        let intent = <Intent as StoredActor>::get(self, signature).ok_or_else(|| {
            format!(
                "cannot build context for intent {signature}: actor \
                     was not found",
            )
        })?;
        let subintents = intent
            .runtime
            .plan
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(|plan| plan.subintents.clone())
            .unwrap_or_default();
        let plan_failures = intent
            .runtime
            .plan_failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Ok(IntentContext {
            signature: signature.clone(),
            content: intent.runtime.content.clone(),
            result: intent.result(),
            step_results: intent
                .runtime
                .steps
                .entries()
                .into_iter()
                .filter_map(|(_, result)| result)
                .collect(),
            subintents,
            plan_failures,
        })
    }

    pub(crate) fn get_intent_content(&self, signature: &IntentSignature) -> Option<String> {
        <Intent as StoredActor>::get(self, signature).map(|intent| intent.runtime.content.clone())
    }
}

// -- Private -- //

impl TaskAccess {
    fn collect_context(
        &self,
        signature: &IntentSignature,
        intents: &mut Vec<IntentContext>,
    ) -> Result<(), String> {
        let context = self.get_intent_context(signature)?;
        if let Some(parent) = signature.parent.as_deref() {
            self.collect_context(parent, intents)?;
        }
        intents.push(context);
        Ok(())
    }
}

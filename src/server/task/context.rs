use marix_common::Actor;
use marix_protocol::{
    Context, ContextChain, IntentContext, IntentSignature, PlanContext, PlanSignature,
};

use super::TaskAccess;
use super::access::StoredActor;
use crate::intent::Intent;
use crate::plan::Plan;

pub(crate) trait ContextSignature {
    fn collect_context(
        &self,
        access: &TaskAccess,
        contexts: &mut Vec<Context>,
    ) -> Result<(), String>;
}

impl TaskAccess {
    pub(crate) fn index_of(&self, signature: &IntentSignature) -> Result<Option<usize>, String> {
        let Some(parent) = signature.parent.as_ref() else {
            return Ok(None);
        };
        let plan = <Plan as StoredActor>::get(self, parent).ok_or_else(|| {
            format!(
                "cannot index intent {signature}: parent plan {parent} \
                 actor was not found",
            )
        })?;
        let intents = plan
            .runtime
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        intents
            .iter()
            .position(|candidate| candidate == signature)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "intent {signature} is not in parent plan {parent}'s \
                     current intents; it may be stale after plan \
                     reconstruction",
                )
            })
    }

    pub(crate) fn get_context_chain<S>(&self, signature: &S) -> Result<ContextChain, String>
    where
        S: ContextSignature,
    {
        let mut contexts = Vec::new();
        signature.collect_context(self, &mut contexts)?;
        Ok(ContextChain { contexts })
    }
}

// -- Private -- //

impl ContextSignature for IntentSignature {
    fn collect_context(
        &self,
        access: &TaskAccess,
        contexts: &mut Vec<Context>,
    ) -> Result<(), String> {
        if let Some(parent) = self.parent.as_ref() {
            access.index_of(self)?;
            parent.collect_context(access, contexts)?;
        }
        contexts.push(Context::Intent(access.intent_context(self)?));
        Ok(())
    }
}

impl ContextSignature for PlanSignature {
    fn collect_context(
        &self,
        access: &TaskAccess,
        contexts: &mut Vec<Context>,
    ) -> Result<(), String> {
        self.intent.collect_context(access, contexts)?;
        contexts.push(Context::Plan(access.plan_context(self)?));
        Ok(())
    }
}

impl TaskAccess {
    fn intent_context(&self, signature: &IntentSignature) -> Result<IntentContext, String> {
        let intent = <Intent as StoredActor>::get(self, signature).ok_or_else(|| {
            format!(
                "cannot build context for intent {signature}: actor \
                     was not found",
            )
        })?;
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
        })
    }

    fn plan_context(&self, signature: &PlanSignature) -> Result<PlanContext, String> {
        let plan = <Plan as StoredActor>::get(self, signature).ok_or_else(|| {
            format!(
                "cannot build context for plan {signature}: actor \
                     was not found",
            )
        })?;
        let signatures = plan
            .runtime
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let intents = signatures
            .iter()
            .map(|intent| self.intent_context(intent))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                format!(
                    "cannot build ordered intents for plan {signature}: \
                     {error}",
                )
            })?;
        let failures = plan
            .runtime
            .failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Ok(PlanContext {
            signature: signature.clone(),
            intents,
            failures,
        })
    }
}

use std::collections::BTreeMap;

use marix_common::external::*;
use marix_protocol::{Context, ContextChain, IntentContext, PlanContext};

use super::RelayRuntime;
use crate::model::ModelRequest;
use crate::prompt::Prompt;

impl RelayRuntime {
    pub(super) fn model_request(&self) -> Result<ModelRequest, String> {
        let chain = match self.signature.plan.as_ref() {
            Some(plan) => self.access.get_context_chain(plan)?,
            None => self.access.get_context_chain(&self.signature.intent)?,
        };
        let tools = {
            let session_context = self.access.session_context()?;
            let context = session_context
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            context.tools.clone()
        };
        Ok(ModelRequest {
            relay: self.signature.clone(),
            system: self.system_prompt()?,
            prompts: self.prompts(&chain)?,
            tools,
        })
    }
}

// -- Private -- //

impl RelayRuntime {
    fn system_prompt(&self) -> Result<String, String> {
        let mut system =
            std::panic::catch_unwind(|| Prompt::load("System")).map_err(|payload| {
                let detail = if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else if let Some(message) = payload.downcast_ref::<&str>() {
                    (*message).to_owned()
                } else {
                    "unknown prompt loading panic".to_owned()
                };
                format!("failed to load System prompt: {detail}")
            })?;
        system.inject("user_request".to_owned(), self.access.user_request.clone());
        system
            .prompt()
            .map_err(|error| format!("failed to render System prompt: {error}"))
    }

    fn context_prompts(&self, chain: &ContextChain) -> Result<Vec<String>, String> {
        if chain.contexts.is_empty() {
            return Err("cannot render an empty context chain".to_owned());
        }

        let mut prompts = Vec::new();
        let mut index = 0;
        while index < chain.contexts.len() {
            let Context::Intent(intent) = &chain.contexts[index] else {
                return Err(format!("context item {} has no preceding goal", index + 1,));
            };
            match chain.contexts.get(index + 1) {
                Some(Context::Plan(plan)) => {
                    if plan.signature.intent.as_ref() != &intent.signature {
                        return Err(format!(
                            "context items {} and {} have different owners",
                            index + 1,
                            index + 2,
                        ));
                    }
                    prompts.push(Self::intent_plan_prompt(intent, plan)?);
                    index += 2;
                }
                Some(Context::Intent(_)) => {
                    return Err(format!(
                        "context item {} is followed by another goal \
                         without ordered goals between them",
                        index + 1,
                    ));
                }
                None => {
                    prompts.push(Self::intent_prompt(intent)?);
                    index += 1;
                }
            }
        }
        Ok(prompts)
    }

    fn prompts(&self, chain: &ContextChain) -> Result<Vec<String>, String> {
        let context_prompts = self.context_prompts(chain)?;
        let mut prompts = Vec::with_capacity(context_prompts.len() + 1);
        prompts.push(self.prompt.clone());
        prompts.extend(context_prompts);
        Ok(prompts)
    }

    fn intent_plan_prompt(intent: &IntentContext, plan: &PlanContext) -> Result<String, String> {
        let goal = Self::json(&intent.content, "goal")?;
        let goals = plan
            .intents
            .iter()
            .map(|intent| intent.content.as_str())
            .collect::<Vec<_>>();
        let goals = Self::json(&goals, "ordered goals")?;
        let mut prompt = format!(
            "To achieve the goal {goal}, the following ordered goals \
             are being followed: {goals}."
        );
        Self::push_calls(&mut prompt, intent)?;
        if !plan.failures.is_empty() {
            let failures = plan
                .failures
                .iter()
                .map(|failure| failure.output.as_str())
                .collect::<Vec<_>>();
            let failures = Self::json(&failures, "previous failed attempts")?;
            prompt.push_str(" previous failed attempts: ");
            prompt.push_str(&failures);
            prompt.push('.');
        }
        Ok(prompt)
    }

    fn intent_prompt(intent: &IntentContext) -> Result<String, String> {
        let goal = Self::json(&intent.content, "current goal")?;
        let mut prompt = format!("The current goal is {goal}.");
        Self::push_calls(&mut prompt, intent)?;
        Ok(prompt)
    }

    fn push_calls(prompt: &mut String, intent: &IntentContext) -> Result<(), String> {
        let calls = Self::calls(intent)?;
        if calls.is_empty() {
            return Ok(());
        }
        let calls = Self::json(&calls, "calls")?;
        prompt.push_str(" calls: ");
        prompt.push_str(&calls);
        prompt.push('.');
        Ok(())
    }

    fn calls(intent: &IntentContext) -> Result<Vec<BTreeMap<String, String>>, String> {
        let mut calls = Vec::new();
        for (result_index, result) in intent.step_results.iter().enumerate() {
            for (line_index, line) in result.output.lines().enumerate() {
                let Some((tool, output)) = line.split_once(": ") else {
                    return Err(format!(
                        "call output {} line {} does not contain `: `",
                        result_index + 1,
                        line_index + 1,
                    ));
                };
                if tool.is_empty() {
                    return Err(format!(
                        "call output {} line {} has an empty tool name",
                        result_index + 1,
                        line_index + 1,
                    ));
                }
                let mut call = BTreeMap::new();
                call.insert(tool.to_owned(), output.to_owned());
                calls.push(call);
            }
        }
        Ok(calls)
    }

    fn json<T>(value: &T, label: &str) -> Result<String, String>
    where
        T: Serialize + ?Sized,
    {
        serde_json::to_string(value).map_err(|error| format!("failed to encode {label}: {error}"))
    }
}

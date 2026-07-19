use std::collections::BTreeMap;

use marix_common::{Arch, Platform, System, external::*};
use marix_protocol::{Context, ContextChain, IntentContext, PlanContext, ToolPreview};

use super::RelayRuntime;
use crate::model::ModelRequest;
use crate::prompt::{MessagePrompt, Prompt, SystemPrompt};

impl RelayRuntime {
    pub(super) fn model_request(&self) -> Result<ModelRequest, String> {
        let chain = match self.signature.plan.as_ref() {
            Some(plan) => self.access.get_context_chain(plan)?,
            None => self.access.get_context_chain(&self.signature.intent)?,
        };
        let message_prompt = MessagePrompt::from_relay_name(&self.signature.name)?;
        let system_prompt = message_prompt.system();
        let native_tool_execution = matches!(message_prompt, MessagePrompt::ToolExecution);
        let needs_tools =
            matches!(system_prompt, SystemPrompt::SystemTools) || native_tool_execution;
        let tools = if needs_tools {
            {
                let session_context = self.access.session_context()?;
                let context = session_context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                context.tools.clone()
            }
        } else {
            Vec::new()
        };
        let native_tools = if native_tool_execution {
            Some(tools.clone())
        } else {
            None
        };
        Ok(ModelRequest {
            relay: self.signature.clone(),
            system: self.system_prompt(system_prompt, &tools)?,
            prompts: self.prompts(&chain)?,
            tools: native_tools,
        })
    }
}

// -- Private -- //

impl RelayRuntime {
    fn system_prompt(
        &self,
        system_prompt: SystemPrompt,
        tools: &[ToolPreview],
    ) -> Result<String, String> {
        let current_system = {
            let session_context = self.access.session_context()?;
            let context = session_context
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            context
                .system
                .ok_or_else(|| "current execution environment is unavailable".to_owned())?
        };
        let template = system_prompt.name();
        let mut system =
            std::panic::catch_unwind(|| Prompt::load(template)).map_err(|payload| {
                let detail = if let Some(message) = payload.downcast_ref::<String>() {
                    message.clone()
                } else if let Some(message) = payload.downcast_ref::<&str>() {
                    (*message).to_owned()
                } else {
                    "unknown prompt loading panic".to_owned()
                };
                format!("failed to load {template} prompt: {detail}")
            })?;
        for parameter in system.parameters() {
            let value = match parameter.as_str() {
                "user_request" => self.access.user_request.clone(),
                "system" => Self::system_text(current_system),
                "tools" => tools
                    .iter()
                    .map(|tool| format!("{}: {}", tool.name, tool.description))
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => {
                    return Err(format!(
                        "unsupported {template} prompt parameter \
                         `{parameter}`"
                    ));
                }
            };
            system.inject(parameter, value);
        }
        system
            .prompt()
            .map_err(|error| format!("failed to render {template} prompt: {error}"))
    }

    fn system_text(system: System) -> String {
        let platform = match system.platform {
            Platform::All => "all supported operating systems",
            Platform::Win => "Windows",
            Platform::Ubuntu => "Ubuntu",
        };
        let arch = match system.arch {
            Arch::All => "all supported 64-bit architectures",
            Arch::Amd => "amd64",
            Arch::Arm => "arm",
        };
        format!("{platform} on {arch}")
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
        Self::tool_calls(&mut prompt, intent)?;
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
        Self::tool_calls(&mut prompt, intent)?;
        Ok(prompt)
    }

    fn tool_calls(prompt: &mut String, intent: &IntentContext) -> Result<(), String> {
        let mut calls = Vec::new();
        for result in &intent.step_results {
            for call_result in &result.calls {
                let mut call = BTreeMap::new();
                call.insert(call_result.tool.clone(), call_result.result.output.clone());
                calls.push(call);
            }
        }
        if calls.is_empty() {
            return Ok(());
        }
        let calls = Self::json(&calls, "calls")?;
        prompt.push_str(" calls: ");
        prompt.push_str(&calls);
        prompt.push('.');
        Ok(())
    }

    fn json<T>(value: &T, label: &str) -> Result<String, String>
    where
        T: Serialize + ?Sized,
    {
        serde_json::to_string(value).map_err(|error| format!("failed to encode {label}: {error}"))
    }
}

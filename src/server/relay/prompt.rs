use std::collections::{BTreeMap, BTreeSet};

use marix_common::{Arch, Platform, System, external::*};
use marix_protocol::{
    ContextChain, IntentContext, ToolPreview, WorkflowComplete, WorkflowInfeasible, WorkflowPlan,
    WorkflowTool,
};

use super::RelayRuntime;
use crate::model::ModelRequest;
use crate::prompt::Prompt;

impl RelayRuntime {
    pub(super) fn model_request(&self) -> Result<ModelRequest, String> {
        let chain = self.access.get_context_chain(&self.signature.intent)?;
        let (current_system, tools) = {
            let session_context = self.access.session_context()?;
            let context = session_context
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let current_system = context
                .system
                .ok_or_else(|| "current execution environment is unavailable".to_owned())?;
            (current_system, context.tools.clone())
        };
        let tools = self.merge_workflow(tools)?;
        Ok(ModelRequest {
            relay: self.signature.clone(),
            system: self.system_prompt(current_system)?,
            prompts: self.context_prompts(&chain)?,
            tools: Some(tools),
        })
    }
}

// -- Private -- //

impl RelayRuntime {
    fn merge_workflow(
        &self,
        execution_tools: Vec<ToolPreview>,
    ) -> Result<Vec<ToolPreview>, String> {
        let mut names = BTreeSet::new();
        for tool in &execution_tools {
            if !names.insert(tool.name.clone()) {
                return Err(format!(
                    "relay `{}` cannot send duplicate execution tool name `{}`",
                    self.signature.name, tool.name,
                ));
            }
        }
        let workflow_tools = [
            WorkflowPlan::preview(),
            WorkflowComplete::preview(),
            WorkflowInfeasible::preview(),
        ];
        for tool in &workflow_tools {
            if names.contains(&tool.name) {
                return Err(format!(
                    "relay `{}` execution tool name `{}` conflicts with \
                     server workflow tool `{}`",
                    self.signature.name, tool.name, tool.name,
                ));
            }
        }
        let mut tools =
            Vec::with_capacity(workflow_tools.len() + execution_tools.len());
        tools.extend(workflow_tools);
        tools.extend(execution_tools);
        Ok(tools)
    }

    fn system_prompt(&self, current_system: System) -> Result<String, String> {
        let template = "System";
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
        if chain.intents.is_empty() {
            return Err("cannot render an empty context chain".to_owned());
        }

        let mut prompts = Vec::with_capacity(chain.intents.len() + 1);
        prompts.push(self.prompt.clone());
        for intent in &chain.intents {
            prompts.push(self.intent_prompt(intent)?);
        }
        Ok(prompts)
    }

    fn intent_prompt(&self, intent: &IntentContext) -> Result<String, String> {
        let goal = Self::json(&intent.content, "current goal")?;
        let mut prompt = format!("The current goal is {goal}.");
        Self::tool_calls(&mut prompt, intent)?;
        if !intent.subintents.is_empty() {
            let subintents = intent
                .subintents
                .iter()
                .map(|signature| self.access.get_intent_context(signature))
                .collect::<Result<Vec<_>, _>>()?;
            let goals = subintents
                .iter()
                .map(|subintent| subintent.content.as_str())
                .collect::<Vec<_>>();
            let results = subintents
                .iter()
                .map(|subintent| subintent.result.as_ref())
                .collect::<Vec<_>>();
            prompt.push_str(" Current ordered subintent goals: ");
            prompt.push_str(&Self::json(&goals, "subintent goals")?);
            prompt.push_str(". Their corresponding results: ");
            prompt.push_str(&Self::json(&results, "subintent results")?);
            prompt.push('.');
        }
        if !intent.plan_failures.is_empty() {
            prompt.push_str(" Previous plan failures: ");
            prompt.push_str(&Self::json(&intent.plan_failures, "plan failures")?);
            prompt.push('.');
        }
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

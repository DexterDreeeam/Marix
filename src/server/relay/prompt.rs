use std::collections::BTreeSet;

use marix_common::{Arch, Platform, System};
use marix_protocol::{
    ContextChain, IntentContext, IntentResult, IntentResultKind, RelayKind, ToolPreview,
    WorkflowCallSummary, WorkflowComplete, WorkflowInfeasible, WorkflowPlan, WorkflowTool,
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
            WorkflowCallSummary::preview(),
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
        let mut tools = Vec::with_capacity(workflow_tools.len() + execution_tools.len());
        tools.extend(workflow_tools);
        for mut tool in execution_tools {
            if let Ok(mut schema) = marix_common::external::serde_json::from_str::<
                marix_common::external::serde_json::Value,
            >(&tool.input)
            {
                if let Some(props) = schema.get_mut("properties").and_then(|v| v.as_object_mut()) {
                    props.insert("purpose".to_owned(), marix_common::external::serde_json::json!({
                        "type": "string",
                        "description": "A short summary of what this tool invocation is doing and why."
                    }));
                }
                if let Some(required) = schema.get_mut("required").and_then(|v| v.as_array_mut()) {
                    required.push(marix_common::external::serde_json::json!("purpose"));
                }
                tool.input = marix_common::external::serde_json::to_string(&schema)
                    .unwrap_or_else(|_| tool.input);
            }
            tools.push(tool);
        }
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
        let Some((current, ancestors)) = chain.intents.split_last() else {
            return Err("cannot render an empty context chain".to_owned());
        };
        if !current.subintents.is_empty() {
            return Err("intent verdict target still has an active plan; \
                 context chain is inconsistent"
                .to_owned());
        }

        let mut prompts = vec![self.workflow_policy_prompt()?];
        if !ancestors.is_empty() {
            let mut context = "[BACKGROUND CONTEXT]\nThese are the parent tasks and their execution history. They are provided for reference only.\n\n\n".to_owned();
            context.push_str(
                &ancestors
                    .iter()
                    .map(|intent| self.plan_prompt(intent))
                    .collect::<Result<Vec<_>, _>>()?
                    .join("\n\n\n"),
            );
            prompts.push(context);
        }
        prompts.push(Self::pending_intent_prompt(current));
        if let RelayKind::ToolCallSummarize { tool, output, .. } = &self.kind {
            prompts.push(self.tool_call_prompt(tool, output)?);
        }
        Ok(prompts)
    }

    fn workflow_policy_prompt(&self) -> Result<String, String> {
        let template = "WorkflowPolicy";
        let mut prompt =
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
        for parameter in prompt.parameters() {
            let value = match parameter.as_str() {
                "goal" => self.access.user_request.clone(),
                _ => {
                    return Err(format!(
                        "unsupported {template} prompt parameter `{parameter}`"
                    ));
                }
            };
            prompt.inject(parameter, value);
        }
        prompt
            .prompt()
            .map_err(|error| format!("failed to render {template} prompt: {error}"))
    }

    /// Renders an ancestor Intent that currently holds an active Plan.
    fn plan_prompt(&self, intent: &IntentContext) -> Result<String, String> {
        let mut prompt = format!("Goal: {}", intent.content);
        Self::append_result(&mut prompt, &intent.result);
        self.append_plan(&mut prompt, intent)?;
        Self::append_tool_calls(&mut prompt, intent);
        Self::append_plan_failures(&mut prompt, intent);
        Ok(prompt)
    }

    /// Renders the Intent currently awaiting a decision (it has no active Plan).
    fn pending_intent_prompt(intent: &IntentContext) -> String {
        let mut prompt = "[CURRENT TASK]\nThis is the task you are executing NOW. Everything you do MUST be scoped strictly to this goal alone."
            .to_owned();
        prompt.push_str(&format!("\nGoal: {}", intent.content));
        Self::append_tool_calls(&mut prompt, intent);
        Self::append_plan_failures(&mut prompt, intent);
        prompt
    }

    /// Renders the trailing message for a `ToolCallSummarize` relay, appended
    /// after the pending intent prompt so the shared prefix stays identical
    /// to a normal decision call for the same intent state.
    fn tool_call_prompt(&self, tool: &str, output: &str) -> Result<String, String> {
        let template = "ToolCallSummarize";
        let mut prompt =
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
        for parameter in prompt.parameters() {
            let value = match parameter.as_str() {
                "tool" => tool.to_owned(),
                "output" => output.replace("\n", "\n      "),
                _ => {
                    return Err(format!(
                        "unsupported {template} prompt parameter `{parameter}`"
                    ));
                }
            };
            prompt.inject(parameter, value);
        }
        prompt
            .prompt()
            .map_err(|error| format!("failed to render {template} prompt: {error}"))
    }

    fn append_result(prompt: &mut String, result: &Option<IntentResult>) {
        if let Some(result) = result {
            prompt.push_str("\nResult: ");
            prompt.push_str(Self::intent_result_status(&result.kind));
            prompt.push_str(" — ");
            prompt.push_str(&result.output);
        }
    }

    fn append_plan(&self, prompt: &mut String, intent: &IntentContext) -> Result<(), String> {
        if intent.subintents.is_empty() {
            return Ok(());
        }

        let subintents = intent
            .subintents
            .iter()
            .map(|signature| self.access.get_intent_context(signature))
            .collect::<Result<Vec<_>, _>>()?;
        prompt.push_str("\nPlan:");
        let mut current_item = None;
        for (index, subintent) in subintents.iter().enumerate() {
            prompt.push_str(&format!("\n{}. {}", index + 1, subintent.content));
            match &subintent.result {
                Some(result) => {
                    let output = result.output.replace("\n", "\n      ");
                    prompt.push_str(&format!("\n   Result:\n      {}", output));
                }
                None => {
                    current_item.get_or_insert(index + 1);
                }
            }
        }
        if let Some(item) = current_item {
            prompt.push_str(&format!("\nCurrently executing item {item} of the plan."));
        }
        Ok(())
    }

    fn append_tool_calls(prompt: &mut String, intent: &IntentContext) {
        let has_calls = intent
            .step_results
            .iter()
            .any(|result| !result.calls.is_empty());
        if !has_calls {
            return;
        }

        prompt.push_str("\nTool calls:");
        let mut index = 1;
        for step_result in &intent.step_results {
            for call in &step_result.calls {
                let purpose = marix_common::external::serde_json::from_str::<
                    marix_common::external::serde_json::Value,
                >(&call.input)
                .ok()
                .and_then(|v| {
                    v.get("purpose")
                        .and_then(|p| p.as_str())
                        .map(|s| s.to_owned())
                })
                .unwrap_or_default();
                let purpose_str = if purpose.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", purpose)
                };

                let output = call.result.output.replace("\n", "\n      ");
                prompt.push_str(&format!(
                    "\n{}. {}{}\n   Result:\n      {}",
                    index, call.tool, purpose_str, output,
                ));
                index += 1;
            }
        }
    }

    fn append_plan_failures(prompt: &mut String, intent: &IntentContext) {
        if intent.plan_failures.is_empty() {
            return;
        }

        prompt.push_str("\nPrevious plan failures:\n");
        let mut failures = Vec::new();
        for failure in &intent.plan_failures {
            let mut failed_goal = String::new();
            for (index, result) in failure.results.iter().enumerate() {
                if let Some(res) = result {
                    match res.kind {
                        IntentResultKind::Failed | IntentResultKind::Infeasible => {
                            failed_goal = failure.goals.get(index).cloned().unwrap_or_default();
                            break;
                        }
                        _ => {}
                    }
                }
            }

            failures.push(marix_common::external::serde_json::json!({
                "goals": failure.goals,
                "failed": failed_goal,
                "reason": failure.reason
            }));
        }
        if let Ok(json_str) = marix_common::external::serde_json::to_string_pretty(&failures) {
            prompt.push_str(&json_str);
        }
    }

    fn intent_result_status(kind: &IntentResultKind) -> &'static str {
        match kind {
            IntentResultKind::Succeed => "succeeded",
            IntentResultKind::Infeasible => "was infeasible",
            IntentResultKind::Canceled => "was canceled",
            IntentResultKind::Failed => "failed",
        }
    }
}

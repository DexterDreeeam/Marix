use std::collections::BTreeSet;

use marix_common::external::*;
use marix_common::{Arch, Platform, System};
use marix_protocol::{
    ContextChain, IntentContext, RelayKind, ToolPreview, WorkflowComplete, WorkflowContinuation,
    WorkflowInfeasible, WorkflowPlan, WorkflowTool,
};

use super::RelayRuntime;
use crate::model::ModelRequest;
use crate::prompt::{Prompt, PromptProfile};

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
        let prompts = self.context_prompts(&chain)?;
        let profile = self.prompt_profile();
        Ok(ModelRequest {
            relay: self.signature.clone(),
            profile,
            system: self.system_prompt(current_system)?,
            prompts,
            tools: Some(tools),
        })
    }
}

// -- Private -- //

impl RelayRuntime {
    fn prompt_profile(&self) -> PromptProfile {
        match &self.kind {
            RelayKind::IntentAnalyze => PromptProfile::ToolMandatory,
            RelayKind::ToolCallSummarize { .. } => {
                PromptProfile::ToolCallSummary
            }
        }
    }

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
            // WorkflowContinuation::preview(), // Server-driven; not exposed to models.
            WorkflowPlan::preview(),
            WorkflowComplete::preview(),
            WorkflowInfeasible::preview(),
        ];
        if names.contains(WorkflowContinuation::NAME) {
            return Err(format!(
                "relay `{}` execution tool name `{}` conflicts with \
                 hidden server workflow tool `{}`",
                self.signature.name,
                WorkflowContinuation::NAME,
                WorkflowContinuation::NAME,
            ));
        }
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

    fn render_prompt(
        name: &str,
        loader: fn(&str) -> Prompt,
        parameters: &[(&str, String)],
    ) -> Result<String, String> {
        let mut prompt =
            std::panic::catch_unwind(|| loader(name)).map_err(|payload| {
                let detail = if let Some(message) =
                    payload.downcast_ref::<String>()
                {
                    message.clone()
                } else if let Some(message) =
                    payload.downcast_ref::<&str>()
                {
                    (*message).to_owned()
                } else {
                    "unknown prompt loading panic".to_owned()
                };
                format!("failed to load prompt {name}: {detail}")
            })?;
        for parameter in prompt.parameters() {
            let value = parameters
                .iter()
                .find(|(name, _)| *name == parameter.as_str())
                .map(|(_, value)| value.clone())
                .ok_or_else(|| {
                    format!(
                        "unsupported prompt {name} parameter \
                         `{parameter}`"
                    )
                })?;
            prompt.inject(parameter, value);
        }
        prompt
            .prompt()
            .map_err(|error| {
                format!("failed to render prompt {name}: {error}")
            })
    }

    fn system_prompt(&self, current_system: System) -> Result<String, String> {
        let template = "System";
        Self::render_prompt(
            template,
            Prompt::load,
            &[("system", Self::system_text(current_system))],
        )
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
        if let RelayKind::ToolCallSummarize {
            tool,
            output,
            continuation_cursor,
            previous_summaries,
            ..
        } = &self.kind
        {
            prompts.push(self.tool_call_prompt(
                tool,
                output,
                continuation_cursor.as_deref(),
                previous_summaries,
            )?);
        }
        Ok(prompts)
    }

    fn workflow_policy_prompt(&self) -> Result<String, String> {
        let template = "WorkflowPolicy";
        Self::render_prompt(
            template,
            Prompt::load,
            &[("goal", self.access.user_request.clone())],
        )
    }

    /// Renders an ancestor Intent that currently holds an active Plan.
    fn plan_prompt(&self, intent: &IntentContext) -> Result<String, String> {
        let mut prompt =
            format!("[**GOAL**]:\n{}\n\n[**PLAN**]:", intent.content);
        self.append_plan(&mut prompt, intent)?;
        Self::append_tool_calls(&mut prompt, intent, "\n\n");
        Self::append_plan_failures(&mut prompt, intent);
        Ok(prompt)
    }

    /// Renders the Intent currently awaiting a decision (it has no active Plan).
    fn pending_intent_prompt(intent: &IntentContext) -> String {
        let mut prompt = "[**CURRENT TASK**]\nThis is the task you are executing NOW. Everything you do MUST be scoped strictly to this goal alone."
            .to_owned();
        prompt.push_str(&format!("\n\n[**GOAL**]:\n{}", intent.content));
        Self::append_tool_calls(&mut prompt, intent, "\n\n");
        Self::append_plan_failures(&mut prompt, intent);
        prompt
    }

    /// Renders the trailing message for a `ToolCallSummarize` relay, appended
    /// after the pending intent prompt so the shared prefix stays identical
    /// to a normal decision call for the same intent state.
    fn tool_call_prompt(
        &self,
        tool: &str,
        output: &str,
        continuation_cursor: Option<&str>,
        previous_summaries: &[String],
    ) -> Result<String, String> {
        let template = if continuation_cursor.is_some() {
            "ToolCallSummarizeWithCursor"
        } else {
            "ToolCallSummarize"
        };
        let pre_chunk =
            Self::pre_chunk_text(tool, previous_summaries)?;
        let mut parameters = vec![
            ("tool", tool.to_owned()),
            ("output", output.to_owned()),
            ("pre_chunk", pre_chunk),
        ];
        if let Some(continuation_cursor) = continuation_cursor {
            parameters.push((
                "continuation_cursor",
                continuation_cursor.to_owned(),
            ));
        }
        Self::render_prompt(template, Prompt::load, &parameters)
    }

    /// Renders the summaries already collected from earlier chunks of the
    /// same tool output, so a later chunk is not summarized blind. Stays
    /// empty until an earlier chunk has been summarized, which keeps the
    /// first-chunk and unchunked renders byte-identical to a template
    /// without this block.
    fn pre_chunk_text(
        tool: &str,
        previous_summaries: &[String],
    ) -> Result<String, String> {
        if previous_summaries.is_empty() {
            return Ok(String::new());
        }
        let mut text = format!("[PRE-CHUNK: {tool}]\n");
        for (index, summary) in previous_summaries.iter().enumerate() {
            text.push_str(&format!("{}. {summary}\n", index + 1));
        }
        let notice =
            Self::render_prompt("PreChunk", Prompt::load_module, &[])?;
        text.push_str(notice.trim_end());
        text.push_str("\n\n");
        Ok(text)
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
        let current_item = subintents
            .iter()
            .position(|subintent| subintent.result.is_none());
        for (index, subintent) in subintents.iter().enumerate() {
            match &subintent.result {
                Some(result) => {
                    prompt.push_str(&format!(
                        "\n{}. {}:\n{}",
                        index + 1,
                        subintent.content,
                        result.output.trim(),
                    ));
                }
                None if Some(index) == current_item => {
                    prompt.push_str(&format!(
                        "\n{}. [EXECUTING NOW] {}",
                        index + 1,
                        subintent.content,
                    ));
                }
                None => {
                    prompt.push_str(&format!("\n{}. {}", index + 1, subintent.content));
                }
            }
        }
        Ok(())
    }

    fn append_tool_calls(
        prompt: &mut String,
        intent: &IntentContext,
        separator: &str,
    ) {
        let has_calls = intent
            .step_results
            .iter()
            .any(|result| !result.calls.is_empty());
        if !has_calls {
            return;
        }

        prompt.push_str(separator);
        prompt.push_str("[**BACKGROUND**]:");
        let mut index = 1;
        for step_result in &intent.step_results {
            for call in &step_result.calls {
                let descriptor = Self::call_descriptor(&call.tool, &call.input);
                let output = Self::single_line(&call.result.output);
                prompt.push_str(&format!(
                    "\n{}. {}{}:\n{}",
                    index, call.tool, descriptor, output,
                ));
                index += 1;
            }
        }
    }

    fn call_descriptor(tool: &str, input: &str) -> String {
        let Ok(serde_json::Value::Object(input)) = serde_json::from_str::<serde_json::Value>(input)
        else {
            return String::new();
        };
        let purpose = input
            .get("purpose")
            .and_then(serde_json::Value::as_str)
            .map(Self::single_line)
            .filter(|value| !value.is_empty());
        if tool.starts_with("workflow_") {
            return purpose
                .map(|value| format!(" ({value})"))
                .unwrap_or_default();
        }

        let mut core = input
            .iter()
            .filter(|(name, _)| name.as_str() != "purpose")
            .collect::<Vec<_>>();
        core.sort_unstable_by(|left, right| left.0.cmp(right.0));
        let core = match core.as_slice() {
            [] => None,
            [(_, value)] => Self::single_parameter_text(value),
            _ => core
                .iter()
                .map(|(name, value)| {
                    Self::quoted_parameter_text(value).map(|value| format!("--{name} {value}"))
                })
                .collect::<Option<Vec<_>>>()
                .map(|values| values.join(" ")),
        };
        let core = core.filter(|value| !value.is_empty());
        let descriptor = match (core, purpose) {
            (Some(core), Some(purpose)) => {
                let core_length = core.chars().count();
                let purpose_length = purpose.chars().count();
                if core_length < purpose_length || core_length < 32 {
                    Some(core)
                } else {
                    Some(purpose)
                }
            }
            (Some(core), None) => Some(core),
            (None, purpose) => purpose,
        };
        descriptor
            .map(|value| format!(" ({value})"))
            .unwrap_or_default()
    }

    fn single_parameter_text(value: &serde_json::Value) -> Option<String> {
        match value {
            serde_json::Value::String(value) => {
                let value = serde_json::to_string(value).ok()?;
                value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .map(str::to_owned)
            }
            _ => serde_json::to_string(value).ok(),
        }
    }

    fn quoted_parameter_text(value: &serde_json::Value) -> Option<String> {
        let value = match value {
            serde_json::Value::String(value) => value.clone(),
            _ => serde_json::to_string(value).ok()?,
        };
        serde_json::to_string(&value).ok()
    }

    fn single_line(value: &str) -> String {
        let mut output = String::with_capacity(value.len());
        let mut line_break = false;
        for character in value.chars() {
            if matches!(character, '\r' | '\n') {
                if !line_break {
                    output.push(' ');
                }
                line_break = true;
            } else {
                output.push(character);
                line_break = false;
            }
        }
        output.trim().to_owned()
    }

    fn append_plan_failures(prompt: &mut String, intent: &IntentContext) {
        if intent.plan_failures.is_empty() {
            return;
        }

        prompt.push_str("\n\n[**FAIL PLANS**]\n");
        for (index, failure) in intent.plan_failures.iter().enumerate() {
            if index > 0 {
                prompt.push_str("\n\n");
            }
            for goal in &failure.goals {
                prompt.push_str(&format!("- {goal}\n"));
            }
            prompt.push_str(&format!("(Failed Reason) {}", failure.reason));
        }
    }
}

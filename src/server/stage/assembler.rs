use std::collections::{BTreeMap, BTreeSet};

use marix_common::external::*;
use marix_common::{Arch, Platform, System};
use marix_protocol::{IntentContext, IntentResultKind, RelaySignature, ToolPreview};

use super::{PromptInjection, PromptParameter, StageType};
use crate::model::ModelRequest;
use crate::prompt::Prompt;
use crate::task::TaskAccess;

pub(crate) struct StageAssembler {
    stage_type: StageType,
    parameters: Vec<PromptParameter>,
    injections: BTreeMap<String, Vec<PromptInjection>>,
    template: Prompt,
}

impl StageAssembler {
    pub(crate) fn new(stage_type: StageType) -> Self {
        let parameters = stage_type.parameters();
        let injections = parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), Vec::new()))
            .collect();
        Self {
            stage_type,
            parameters,
            injections,
            template: Prompt::load(stage_type.prompt_name()),
        }
    }

    pub(crate) fn parameters(&self) -> Vec<PromptParameter> {
        self.parameters.clone()
    }

    pub(crate) fn stage_type(&self) -> StageType {
        self.stage_type
    }

    pub(crate) fn inject(
        &mut self,
        name: &str,
        value: String,
        tag: Option<String>,
    ) {
        let parameter = self
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "unknown stage prompt parameter `{name}` for {:?}",
                    self.stage_type,
                )
            });
        if !parameter.repeatable && tag.is_some() {
            panic!(
                "non-repeatable stage prompt parameter `{name}` \
                 cannot carry a tag"
            );
        }
        let injections = self
            .injections
            .get_mut(name)
            .unwrap_or_else(|| unreachable!("known parameter has no slot"));
        if !parameter.repeatable && !injections.is_empty() {
            panic!(
                "non-repeatable stage prompt parameter `{name}` \
                 cannot be injected more than once"
            );
        }
        if let Some(tag) = tag.as_deref()
            && injections
                .iter()
                .any(|injection| injection.tag.as_deref() == Some(tag))
        {
            panic!(
                "stage prompt parameter `{name}` has duplicate tag \
                 `{tag}`"
            );
        }
        injections.push(PromptInjection { value, tag });
    }

    pub(crate) fn assemble(
        &self,
        access: &TaskAccess,
        relay: RelaySignature,
    ) -> Result<ModelRequest, String> {
        let mut injections = self.injections.clone();
        self.inject_context(access, &relay, &mut injections)?;
        self.validate(&injections);
        let mut template = self.template.clone();
        self.apply_injections(&mut template, &injections);
        let prompt = template.prompt().map_err(|error| {
            format!(
                "failed to render stage prompt {}: {error}",
                self.stage_type.prompt_name(),
            )
        })?;
        let (system, tools) = self.environment(access)?;
        Ok(ModelRequest {
            relay,
            profile: self.stage_type.profile(),
            system: Self::system_prompt(system)?,
            prompts: vec![prompt],
            tools: Some(Self::ordinary_tools(tools)?),
        })
    }
}

// -- Private -- //

impl StageAssembler {
    fn inject_context(
        &self,
        access: &TaskAccess,
        relay: &RelaySignature,
        injections: &mut BTreeMap<String, Vec<PromptInjection>>,
    ) -> Result<(), String> {
        let chain = access.get_context_chain(&relay.intent)?;
        let current = chain
            .intents
            .last()
            .ok_or_else(|| "cannot assemble an empty context chain".to_owned())?;
        Self::inject_single(injections, "intent", current.content.clone());
        Self::inject_single(
            injections,
            "tool_call_count",
            current.tool_call_count.to_string(),
        );
        for (intent_index, intent) in chain.intents.iter().enumerate() {
            self.inject_plan(
                access,
                intent,
                intent_index,
                injections,
            )?;
            Self::inject_tool_calls(intent, intent_index, injections);
        }
        Self::inject_subintent_results(
            access,
            current,
            injections,
        )?;
        Self::inject_plan_failures(current, injections);
        Ok(())
    }

    fn inject_plan(
        &self,
        access: &TaskAccess,
        intent: &IntentContext,
        intent_index: usize,
        injections: &mut BTreeMap<String, Vec<PromptInjection>>,
    ) -> Result<(), String> {
        for (index, signature) in intent.subintents.iter().enumerate() {
            let subintent = access.get_intent_context(signature)?;
            let tag = format!("{intent_index:08}:{index:08}");
            let (status, result) = match subintent.result {
                Some(result) => {
                    let status = match result.kind {
                        IntentResultKind::Succeed => "complete",
                        IntentResultKind::Canceled => "canceled",
                        IntentResultKind::Failed => "failed",
                        IntentResultKind::Infeasible => "infeasible",
                    };
                    (status.to_owned(), result.output)
                }
                None => ("executing".to_owned(), String::new()),
            };
            Self::inject_tagged(
                injections,
                "plan_goal",
                subintent.content,
                &tag,
            );
            Self::inject_tagged(
                injections,
                "plan_status",
                status,
                &tag,
            );
            Self::inject_tagged(
                injections,
                "plan_result",
                result,
                &tag,
            );
        }
        Ok(())
    }

    fn inject_tool_calls(
        intent: &IntentContext,
        intent_index: usize,
        injections: &mut BTreeMap<String, Vec<PromptInjection>>,
    ) {
        let mut call_index = 0;
        for step in &intent.step_results {
            for call in &step.calls {
                let tag =
                    format!("{intent_index:08}:{call_index:08}");
                Self::inject_tagged(
                    injections,
                    "tool_name",
                    call.tool.clone(),
                    &tag,
                );
                Self::inject_tagged(
                    injections,
                    "tool_arguments",
                    call.input.clone(),
                    &tag,
                );
                Self::inject_tagged(
                    injections,
                    "tool_result",
                    call.result.output.clone(),
                    &tag,
                );
                call_index += 1;
            }
        }
    }

    fn inject_subintent_results(
        access: &TaskAccess,
        intent: &IntentContext,
        injections: &mut BTreeMap<String, Vec<PromptInjection>>,
    ) -> Result<(), String> {
        for (index, signature) in intent.subintents.iter().enumerate() {
            let subintent = access.get_intent_context(signature)?;
            let Some(result) = subintent.result else {
                continue;
            };
            let tag = format!("{index:08}");
            Self::inject_tagged(
                injections,
                "subintent_goal",
                subintent.content,
                &tag,
            );
            Self::inject_tagged(
                injections,
                "subintent_result",
                result.output,
                &tag,
            );
        }
        Ok(())
    }

    fn inject_plan_failures(
        intent: &IntentContext,
        injections: &mut BTreeMap<String, Vec<PromptInjection>>,
    ) {
        for (plan_index, failure) in
            intent.plan_failures.iter().enumerate()
        {
            for (goal_index, goal) in
                failure.goals.iter().enumerate()
            {
                let tag =
                    format!("{plan_index:08}:{goal_index:08}");
                Self::inject_tagged(
                    injections,
                    "failed_plan_goal",
                    goal.clone(),
                    &tag,
                );
                Self::inject_tagged(
                    injections,
                    "failed_plan_reason",
                    failure.reason.clone(),
                    &tag,
                );
            }
        }
    }

    fn inject_single(
        injections: &mut BTreeMap<String, Vec<PromptInjection>>,
        name: &str,
        value: String,
    ) {
        let Some(values) = injections.get_mut(name) else {
            return;
        };
        if values.is_empty() {
            values.push(PromptInjection { value, tag: None });
        }
    }

    fn inject_tagged(
        injections: &mut BTreeMap<String, Vec<PromptInjection>>,
        name: &str,
        value: String,
        tag: &str,
    ) {
        let Some(values) = injections.get_mut(name) else {
            return;
        };
        values.push(PromptInjection {
            value,
            tag: Some(tag.to_owned()),
        });
    }

    fn validate(
        &self,
        injections: &BTreeMap<String, Vec<PromptInjection>>,
    ) {
        for parameter in &self.parameters {
            let values = &injections[&parameter.name];
            if parameter.required && values.is_empty() {
                panic!(
                    "required stage prompt parameter `{}` has no \
                     injection",
                    parameter.name,
                );
            }
        }
        let mut packs =
            BTreeMap::<&str, Vec<&PromptParameter>>::new();
        for parameter in &self.parameters {
            if parameter.repeatable {
                packs
                    .entry(parameter.pack_tag.as_str())
                    .or_default()
                    .push(parameter);
            }
        }
        for (pack_tag, parameters) in packs {
            self.validate_pack(pack_tag, &parameters, injections);
        }
    }

    fn validate_pack(
        &self,
        pack_tag: &str,
        parameters: &[&PromptParameter],
        injections: &BTreeMap<String, Vec<PromptInjection>>,
    ) {
        let values = parameters
            .iter()
            .flat_map(|parameter| injections[&parameter.name].iter())
            .collect::<Vec<_>>();
        let tags = values
            .iter()
            .filter_map(|injection| injection.tag.as_deref())
            .collect::<BTreeSet<_>>();
        let multiple = tags.len() > 1
            || parameters
                .iter()
                .any(|parameter| {
                    injections[&parameter.name].len() > 1
                });
        if multiple && values.iter().any(|injection| injection.tag.is_none()) {
            panic!(
                "repeatable stage prompt package `{pack_tag}` has \
                 multiple groups but an injection has no tag"
            );
        }
        if !multiple {
            return;
        }
        for parameter in parameters
            .iter()
            .filter(|parameter| parameter.required)
        {
            let parameter_tags = injections[&parameter.name]
                .iter()
                .filter_map(|injection| injection.tag.as_deref())
                .collect::<BTreeSet<_>>();
            if parameter_tags != tags {
                panic!(
                    "required parameter `{}` does not cover every tag \
                     in package `{pack_tag}`",
                    parameter.name,
                );
            }
        }
    }

    fn apply_injections(
        &self,
        template: &mut Prompt,
        injections: &BTreeMap<String, Vec<PromptInjection>>,
    ) {
        for parameter in &self.parameters {
            let values = &injections[&parameter.name];
            let tagged = parameter.repeatable
                && values.iter().all(|injection| {
                    injection.tag.is_some()
                });
            if tagged {
                let tags = self
                    .parameters
                    .iter()
                    .filter(|candidate| {
                        candidate.pack_tag == parameter.pack_tag
                    })
                    .flat_map(|candidate| {
                        injections[&candidate.name].iter()
                    })
                    .filter_map(|injection| injection.tag.as_ref())
                    .collect::<BTreeSet<_>>();
                for tag in tags {
                    let value = values
                        .iter()
                        .find(|injection| {
                            injection.tag.as_ref() == Some(tag)
                        })
                        .map(|injection| injection.value.clone())
                        .unwrap_or_default();
                    template.inject(parameter.name.clone(), value);
                }
            } else {
                for injection in values {
                    template.inject(
                        parameter.name.clone(),
                        injection.value.clone(),
                    );
                }
            }
        }
    }

    fn environment(
        &self,
        access: &TaskAccess,
    ) -> Result<(System, Vec<ToolPreview>), String> {
        let session_context = access.session_context()?;
        let context = session_context
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let system = context.system.ok_or_else(|| {
            "current execution environment is unavailable".to_owned()
        })?;
        Ok((system, context.tools.clone()))
    }

    fn system_prompt(system: System) -> Result<String, String> {
        let mut prompt = Prompt::load("System");
        for parameter in prompt.parameters() {
            let value = match parameter.name.as_str() {
                "system" => Self::system_text(system),
                other => {
                    return Err(format!(
                        "unsupported System prompt parameter `{other}`"
                    ));
                }
            };
            prompt.inject(parameter.name, value);
        }
        prompt.prompt().map_err(|error| {
            format!("failed to render System prompt: {error}")
        })
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

    fn ordinary_tools(
        execution_tools: Vec<ToolPreview>,
    ) -> Result<Vec<ToolPreview>, String> {
        let mut names = BTreeSet::new();
        let mut tools = Vec::with_capacity(execution_tools.len());
        for mut tool in execution_tools {
            if !names.insert(tool.name.clone()) {
                return Err(format!(
                    "cannot send duplicate execution tool name `{}`",
                    tool.name,
                ));
            }
            if let Ok(mut schema) =
                serde_json::from_str::<serde_json::Value>(&tool.input)
            {
                if let Some(properties) = schema
                    .get_mut("properties")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    properties.insert(
                        "purpose".to_owned(),
                        serde_json::json!({
                            "type": "string",
                            "description": "A short summary of what this tool invocation is doing and why."
                        }),
                    );
                }
                if let Some(required) = schema
                    .get_mut("required")
                    .and_then(serde_json::Value::as_array_mut)
                {
                    required.push(serde_json::json!("purpose"));
                }
                tool.input = serde_json::to_string(&schema)
                    .unwrap_or_else(|_| tool.input);
            }
            tools.push(tool);
        }
        Ok(tools)
    }
}

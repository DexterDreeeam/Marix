use std::collections::{BTreeMap, BTreeSet};

use marix_common::external::*;
use marix_protocol::{RelaySignature, StepDraft};

use super::context::inject_context;
use super::environment::request_environment;
use super::StageResult;
use crate::model::ModelRequest;
use crate::prompt::{
    Prompt, PromptInjection, PromptParameter, PromptProfile,
};
use crate::task::TaskAccess;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageType {
    IntentPlanning,
    IntentToolCalling,
    IntentReplan,
    IntentInfeasible,
    IntentSubintentComplete,
    IntentComplete,
}

impl StageType {
    pub(crate) fn profile(self) -> PromptProfile {
        match self {
            Self::IntentToolCalling => PromptProfile::ToolMandatory,
            _ => PromptProfile::StageJson,
        }
    }

    pub(crate) fn parse_result(
        self,
        output: &str,
    ) -> Result<StageResult, String> {
        let result = if matches!(self, Self::IntentToolCalling) {
            StepDraft::parse(output)
                .map(StageResult::NativeToolCalls)
                .map_err(|error| {
                    format!("native tool calls are invalid: {error}")
                })?
        } else {
            serde_json::from_str(output).map_err(|error| {
                format!("stage result is invalid JSON: {error}")
            })?
        };
        let is_valid = matches!(
            (self, &result),
            (
                Self::IntentPlanning | Self::IntentReplan,
                StageResult::Plan { .. } | StageResult::Reject { .. },
            ) | (
                Self::IntentToolCalling,
                StageResult::NativeToolCalls(_),
            ) | (
                Self::IntentInfeasible,
                StageResult::Infeasible { .. }
                    | StageResult::Reject { .. },
            ) | (
                Self::IntentSubintentComplete | Self::IntentComplete,
                StageResult::IntentComplete { .. }
                    | StageResult::Reject { .. },
            )
        );
        if !is_valid {
            return Err(format!(
                "result {result:?} is not valid for stage {:?}",
                self,
            ));
        }
        if let StageResult::Plan { subintents, .. } = &result {
            let minimum = if matches!(self, Self::IntentPlanning) {
                2
            } else {
                1
            };
            if subintents.len() < minimum {
                return Err(format!(
                    "stage {:?} plan must contain at least {minimum} \
                     subintent(s)",
                    self,
                ));
            }
        }
        Ok(result)
    }
}

pub(crate) struct StageAssembler {
    prompt_name: String,
    profile: PromptProfile,
    parameters: Vec<PromptParameter>,
    injections: BTreeMap<String, Vec<PromptInjection>>,
    template: Prompt,
}

impl StageAssembler {
    pub(crate) fn for_intent(stage_type: StageType) -> Self {
        Self::new(
            format!("{stage_type:?}"),
            stage_type.profile(),
        )
    }

    pub(crate) fn new(
        prompt_name: String,
        profile: PromptProfile,
    ) -> Self {
        let template = Prompt::load(&prompt_name);
        let parameters = template.parameters();
        let injections = parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), Vec::new()))
            .collect();
        Self {
            prompt_name,
            profile,
            parameters,
            injections,
            template,
        }
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
                    "unknown stage prompt parameter `{name}` for {}",
                    self.prompt_name,
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
        inject_context(access, &relay, &mut injections)?;
        self.validate(&injections);
        let mut template = self.template.clone();
        self.apply_injections(&mut template, &injections);
        let prompt = template.prompt().map_err(|error| {
            format!(
                "failed to render stage prompt {}: {error}",
                self.prompt_name,
            )
        })?;
        let (system, tools) = request_environment(access)?;
        Ok(ModelRequest {
            relay,
            profile: self.profile,
            system,
            prompts: vec![prompt],
            tools: Some(tools),
        })
    }
}

// -- Private -- //

impl StageAssembler {
    fn validate(
        &self,
        injections: &BTreeMap<String, Vec<PromptInjection>>,
    ) {
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
        for parameter in parameters {
            let mut parameter_tags = BTreeSet::new();
            for tag in injections[&parameter.name]
                .iter()
                .filter_map(|injection| injection.tag.as_deref())
            {
                if !parameter_tags.insert(tag) {
                    panic!(
                        "stage prompt parameter `{}` has duplicate tag \
                         `{tag}` in package `{pack_tag}`",
                        parameter.name,
                    );
                }
            }
        }
        let values = parameters
            .iter()
            .flat_map(|parameter| injections[&parameter.name].iter())
            .collect::<Vec<_>>();
        let tags = values
            .iter()
            .filter_map(|injection| injection.tag.as_deref())
            .collect::<BTreeSet<_>>();
        let multiple = tags.len() > 1
            || parameters.iter().any(|parameter| {
                injections[&parameter.name].len() > 1
            });
        if (!tags.is_empty() || multiple)
            && values
                .iter()
                .any(|injection| injection.tag.is_none())
        {
            panic!(
                "repeatable stage prompt package `{pack_tag}` has \
                 grouped injections but an injection has no tag"
            );
        }
        if tags.is_empty() {
            return;
        }
        for parameter in parameters {
            let parameter_tags = injections[&parameter.name]
                .iter()
                .filter_map(|injection| injection.tag.as_deref())
                .collect::<BTreeSet<_>>();
            if parameter_tags != tags {
                panic!(
                    "parameter `{}` does not cover every tag \
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
        let mut pack_tags =
            BTreeMap::<&str, BTreeSet<&str>>::new();
        for parameter in self
            .parameters
            .iter()
            .filter(|parameter| parameter.repeatable)
        {
            pack_tags
                .entry(parameter.pack_tag.as_str())
                .or_default()
                .extend(
                    injections[&parameter.name]
                        .iter()
                        .filter_map(|injection| {
                            injection.tag.as_deref()
                        }),
                );
        }
        for parameter in &self.parameters {
            let values = &injections[&parameter.name];
            let tags = pack_tags.get(parameter.pack_tag.as_str());
            let tagged = parameter.repeatable
                && tags.is_some_and(|tags| !tags.is_empty())
                && values
                    .iter()
                    .all(|injection| injection.tag.is_some());
            if tagged {
                self.inject_tagged_values(
                    template,
                    parameter,
                    values,
                    tags.unwrap_or_else(|| {
                        unreachable!("tagged parameter has no tags")
                    }),
                );
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

    fn inject_tagged_values(
        &self,
        template: &mut Prompt,
        parameter: &PromptParameter,
        values: &[PromptInjection],
        tags: &BTreeSet<&str>,
    ) {
        for tag in tags {
            let value = values
                .iter()
                .find(|injection| {
                    injection.tag.as_deref() == Some(*tag)
                })
                .unwrap_or_else(|| {
                    unreachable!(
                        "validated parameter `{}` is missing package \
                         tag `{tag}`",
                        parameter.name,
                    )
                });
            template.inject(
                parameter.name.clone(),
                value.value.clone(),
            );
        }
    }
}

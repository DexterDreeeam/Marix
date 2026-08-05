use marix_common::external::*;
use marix_protocol::StepDraft;

use super::{
    PromptParameter, StageResult, StageResultType,
};
use crate::prompt::{Prompt, PromptProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageType {
    IntentPlanning,
    IntentToolCalling,
    IntentReplan,
    IntentInfeasible,
    IntentSubintentComplete,
    IntentComplete,
    InvocationContinue,
}

impl StageType {
    pub(crate) fn prompt_name(self) -> &'static str {
        match self {
            Self::IntentPlanning => "IntentPlanning",
            Self::IntentToolCalling => "IntentToolCalling",
            Self::IntentReplan => "IntentReplan",
            Self::IntentInfeasible => "IntentInfeasible",
            Self::IntentSubintentComplete => {
                "IntentSubintentComplete"
            }
            Self::IntentComplete => "IntentComplete",
            Self::InvocationContinue => "InvocationContinue",
        }
    }

    pub(crate) fn profile(self) -> PromptProfile {
        match self {
            Self::IntentToolCalling => PromptProfile::ToolMandatory,
            _ => PromptProfile::StageJson,
        }
    }

    pub(crate) fn is_intent(self) -> bool {
        !matches!(self, Self::InvocationContinue)
    }

    pub(crate) fn parameters(self) -> Vec<PromptParameter> {
        let mut parameters = Prompt::load(self.prompt_name()).parameters();
        for parameter in &mut parameters {
            parameter.required = self
                .required_parameters()
                .contains(&parameter.name.as_str());
        }
        parameters
    }

    pub(crate) fn result_types(
        self,
    ) -> &'static [StageResultType] {
        use StageResultType::*;
        match self {
            Self::IntentPlanning => &[Plan, Reject],
            Self::IntentToolCalling => &[NativeToolCalls],
            Self::IntentReplan => &[Plan, Reject],
            Self::IntentInfeasible => &[Infeasible, Reject],
            Self::IntentSubintentComplete => {
                &[IntentComplete, Reject]
            }
            Self::IntentComplete => &[IntentComplete, Reject],
            Self::InvocationContinue => {
                &[InvocationContinue, InvocationComplete]
            }
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
        if !self.result_types().contains(&result.result_type()) {
            return Err(format!(
                "result {:?} is not valid for stage {:?}",
                result.result_type(),
                self,
            ));
        }
        if let StageResult::Plan(plan) = &result {
            let minimum = if matches!(self, Self::IntentPlanning) {
                2
            } else {
                1
            };
            if plan.subintents.len() < minimum {
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

// -- Private -- //

impl StageType {
    fn required_parameters(self) -> &'static [&'static str] {
        match self {
            Self::IntentPlanning => &["intent"],
            Self::IntentToolCalling => &["intent"],
            Self::IntentReplan => &["intent"],
            Self::IntentInfeasible => &["intent"],
            Self::IntentSubintentComplete => {
                &["intent", "subintent_goal", "subintent_result"]
            }
            Self::IntentComplete => &["intent", "tool_name", "tool_result"],
            Self::InvocationContinue => &["tool", "output"],
        }
    }
}

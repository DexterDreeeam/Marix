use marix_common::external::*;
use marix_protocol::{
    InvocationRequest, InvocationSignature, InvocationStepKind, ModelStepKind, PlanError,
    RelayRequest, RelaySignature, StepDraft, StepKind, StepSignature,
};

use super::state::StepState;
use crate::prompt::Prompt;

pub(super) fn step_kind(
    signature: &StepSignature,
    draft: &StepDraft,
) -> Result<StepKind, PlanError> {
    match draft.kind.trim() {
        "tool" => Ok(StepKind::Invocation(InvocationStepKind::Invocation(
            InvocationRequest {
                signature: InvocationSignature::new(
                    signature.task.clone(),
                    signature.plan.clone(),
                    signature.clone(),
                    draft.name.clone(),
                ),
                input: draft.input.clone(),
            },
        ))),
        "intent" => Ok(StepKind::Intent),
        "model" => Ok(StepKind::Model(model_step_kind(draft)?)),
        kind => Err(PlanError::InvalidStepKind(kind.to_owned())),
    }
}

fn model_step_kind(draft: &StepDraft) -> Result<ModelStepKind, PlanError> {
    parse_model_step_name(&draft.name)
        .or_else(|| parse_model_step_name(input_model_name(&draft.input)))
        .ok_or_else(|| PlanError::InvalidModelStep {
            name: draft.name.clone(),
            input: draft.input.clone(),
        })
}

fn parse_model_step_name(name: &str) -> Option<ModelStepKind> {
    match name.trim() {
        "Initial" | "initial" => Some(ModelStepKind::Initial),
        "Analysis" | "analysis" => Some(ModelStepKind::Analysis),
        _ => None,
    }
}

fn input_model_name(input: &str) -> &str {
    input.split(',').next().unwrap_or_default().trim()
}

pub(super) fn model_request(
    state: &StepState,
    model_kind: ModelStepKind,
) -> Result<RelayRequest, String> {
    let prompt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<String, String> {
            let name = match model_kind {
                ModelStepKind::Initial => "Initial",
                ModelStepKind::Analysis => "Analysis",
            };
            let mut prompt = Prompt::load(name);
            let session_context = state
                .access
                .session_context
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .snapshot();
            let user_request = state.access.user_request.clone();
            let input = state
                .input
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
                .map(|input| {
                    serde_json::from_str::<serde_json::Value>(&input).map_err(|error| {
                        format!(
                            "model step {} input is invalid JSON: {error}",
                            &state.signature,
                        )
                    })
                })
                .transpose()?;

            for parameter in prompt.parameters() {
                let value = match parameter.as_str() {
                    "tools" => {
                        serde_json::to_string(&session_context.tools).unwrap_or_else(|error| {
                            panic!("failed to serialize prompt tools: {error}")
                        })
                    }
                    "user_request" => user_request.clone(),
                    "background" | "call_output" => match input.as_ref() {
                        None => String::new(),
                        Some(input) => {
                            let object = input.as_object().ok_or_else(|| {
                                format!(
                                    "model step {} input must be a JSON object",
                                    &state.signature,
                                )
                            })?;
                            let value = object.get(&parameter).ok_or_else(|| {
                                format!(
                                    "model step {} input field `{parameter}` is missing",
                                    &state.signature,
                                )
                            })?;
                            value.as_str().map(str::to_owned).ok_or_else(|| {
                                format!(
                                    "model step {} input field `{parameter}` must be a string",
                                    &state.signature,
                                )
                            })?
                        }
                    },
                    _ => {
                        return Err(format!(
                            "prompt `{name}` requires unsupported parameter `{parameter}`"
                        ));
                    }
                };
                prompt.inject(parameter, value);
            }

            prompt
                .prompt()
                .map_err(|error| format!("prompt `{name}` rendering failed: {error}"))
        },
    ))
    .map_err(|payload| {
        let detail = if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else {
            "unknown prompt construction panic".to_owned()
        };
        format!("model prompt construction failed: {detail}")
    })??;
    let signature = RelaySignature::new(
        state.signature.task.clone(),
        state.signature.plan.clone(),
        state.signature.clone(),
        state.signature.name.clone(),
    );
    Ok(RelayRequest { signature, prompt })
}

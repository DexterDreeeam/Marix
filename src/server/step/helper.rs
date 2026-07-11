use marix_protocol::{
    InvocationRequest, InvocationSignature, InvocationStepKind, ModelStepKind, PlanError,
    RelayRequest, RelaySignature, StepDraft, StepKind, StepSignature, ToolInputSchema,
};

use super::state::StepState;
use crate::prompt::{AnalysisPrompt, InitialPrompt, Prompt};

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
                input: ToolInputSchema {
                    content: draft.input.clone(),
                },
            },
        ))),
        "intent" => Ok(StepKind::Intent),
        "model" => Ok(StepKind::Model(model_step_kind(draft)?)),
        kind => Err(PlanError::InvalidStepKind(kind.to_owned())),
    }
}

pub(super) fn model_step_kind(draft: &StepDraft) -> Result<ModelStepKind, PlanError> {
    parse_model_step_name(&draft.name)
        .or_else(|| parse_model_step_name(input_model_name(&draft.input)))
        .ok_or_else(|| PlanError::InvalidModelStep {
            name: draft.name.clone(),
            input: draft.input.clone(),
        })
}

pub(super) fn parse_model_step_name(name: &str) -> Option<ModelStepKind> {
    match name.trim() {
        "Initial" | "initial" => Some(ModelStepKind::Initial),
        "Analysis" | "analysis" => Some(ModelStepKind::Analysis),
        _ => None,
    }
}

pub(super) fn input_model_name(input: &str) -> &str {
    input.split(',').next().unwrap_or_default().trim()
}

pub(super) fn model_request(
    state: &StepState,
    model_kind: ModelStepKind,
) -> Result<RelayRequest, String> {
    let prompt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<String, String> {
            let session_context = state
                .access
                .session_context
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .snapshot();
            match model_kind {
                ModelStepKind::Initial => Ok(InitialPrompt::new(
                    state.access.user_request.clone(),
                    session_context,
                )
                .prompt()),
                ModelStepKind::Analysis => {
                    let execution_output = state
                        .input
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .clone()
                        .ok_or_else(|| {
                            format!(
                                "analysis model step {} input is unavailable",
                                &state.signature,
                            )
                        })?;
                    Ok(AnalysisPrompt::new(
                        state.access.user_request.clone(),
                        execution_output,
                        String::new(),
                        String::new(),
                        session_context,
                    )
                    .prompt())
                }
            }
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

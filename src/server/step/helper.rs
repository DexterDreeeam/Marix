use marix_protocol::{
    InvocationRequest, InvocationSignature, InvocationStepKind, ModelStepKind,
    PlanError, StepDraft, StepKind, StepSignature, ToolInputSchema,
};

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

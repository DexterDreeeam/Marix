use marix_common::external::*;
use marix_protocol::{
    ExecutionRequest, ExecutionSignature, ExecutionStepKind, Plan, StepDraft, StepKind,
    TaskSignature, ToolInputSchema,
};

/// Converts a model planning response into a protocol [`Plan`].
///
/// The planning prompts emit a simplified plan where an execution invocation is
/// a `{ "tool", "input" }` pair and carries no execution signature. This is the
/// conversion layer the prompts refer to: it assigns each invocation a real
/// [`ExecutionSignature`] derived from the owning task, so the model never has
/// to produce internal identifiers. Returns `None` when the content is not a
/// plan (for example a bare answer object).
pub(crate) fn parse_plan(content: &str, task: &TaskSignature) -> Option<Plan> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    let object = value.as_object()?;
    let description = object.get("description")?.as_str()?.to_owned();
    let expected_result = object
        .get("expected_result")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Some(Plan {
        description,
        run_steps: convert_steps(object.get("run_steps"), task),
        pending_steps: convert_steps(object.get("pending_steps"), task),
        expected_result,
    })
}

// -- Private -- //

fn convert_steps(value: Option<&serde_json::Value>, task: &TaskSignature) -> Vec<StepDraft> {
    let Some(array) = value.and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|step| convert_step(step, task))
        .collect()
}

fn convert_step(value: &serde_json::Value, task: &TaskSignature) -> Option<StepDraft> {
    let kind = convert_kind(value.get("kind")?, task)?;
    let description = value
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Some(StepDraft { kind, description })
}

fn convert_kind(value: &serde_json::Value, task: &TaskSignature) -> Option<StepKind> {
    if let Some(text) = value.as_str() {
        return (text == "Intent").then_some(StepKind::Intent);
    }
    let object = value.as_object()?;
    if let Some(execution) = object.get("Execution") {
        return convert_execution(execution, task).map(StepKind::Execution);
    }
    if let Some(model) = object.get("Model") {
        return serde_json::from_value(model.clone())
            .ok()
            .map(StepKind::Model);
    }
    if let Some(user) = object.get("User") {
        return serde_json::from_value(user.clone())
            .ok()
            .map(StepKind::User);
    }
    None
}

fn convert_execution(value: &serde_json::Value, task: &TaskSignature) -> Option<ExecutionStepKind> {
    if let Some(text) = value.as_str() {
        return match text {
            "Cancel" => Some(ExecutionStepKind::Cancel),
            "Kill" => Some(ExecutionStepKind::Kill),
            _ => None,
        };
    }
    let invocation = value.as_object()?.get("Invocation")?;
    let tool = invocation.get("tool")?.as_str()?.to_owned();
    let input = invocation
        .get("input")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Some(ExecutionStepKind::Invocation(ExecutionRequest {
        signature: ExecutionSignature::new(task.clone(), tool),
        input: ToolInputSchema { content: input },
    }))
}

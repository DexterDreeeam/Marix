use marix_common::external::*;
use marix_protocol::{
    ExecutionRequest, ExecutionSignature, ExecutionStepKind, Plan, StepDraft, StepKind,
    TaskSignature, ToolInputSchema,
};

/// Converts a model planning response into a protocol [`Plan`].
///
/// The planning prompts emit a simplified plan: `run_steps` are concrete tool
/// calls given as `{ "tool": { "name", "description", "input" } }`, and
/// `pending_steps` are future intentions given as `{ "intent": ... }`. This is
/// the conversion layer the prompts refer to: it assigns each run step a real
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
        run_steps: convert_run_steps(object.get("run_steps"), task),
        pending_steps: convert_pending_steps(object.get("pending_steps")),
        expected_result,
    })
}

// -- Private -- //

/// Builds the concrete run steps. Each entry is
/// `{ "tool": { "name", "description", "input" } }` and becomes an execution
/// invocation whose signature is derived from `task`.
fn convert_run_steps(value: Option<&serde_json::Value>, task: &TaskSignature) -> Vec<StepDraft> {
    let Some(array) = value.and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|step| convert_run_step(step, task))
        .collect()
}

fn convert_run_step(value: &serde_json::Value, task: &TaskSignature) -> Option<StepDraft> {
    let tool = value.as_object()?.get("tool")?.as_object()?;
    let name = tool.get("name")?.as_str()?.to_owned();
    let input = tool
        .get("input")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let description = tool
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Some(StepDraft {
        kind: StepKind::Execution(ExecutionStepKind::Invocation(ExecutionRequest {
            signature: ExecutionSignature::new(task.clone(), name),
            input: ToolInputSchema { content: input },
        })),
        description,
    })
}

/// Builds the pending intentions. Each entry is `{ "intent": ... }` and becomes
/// an [`StepKind::Intent`] draft that a later analysis step can make concrete.
fn convert_pending_steps(value: Option<&serde_json::Value>) -> Vec<StepDraft> {
    let Some(array) = value.and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    array.iter().filter_map(convert_pending_step).collect()
}

fn convert_pending_step(value: &serde_json::Value) -> Option<StepDraft> {
    let object = value.as_object()?;
    let intent = object.get("intent")?.as_str()?.to_owned();
    Some(StepDraft {
        kind: StepKind::Intent,
        description: intent,
    })
}

use marix_protocol::{PlanDraft, StepDraft};

use crate::step::Step;

pub(crate) fn initial_plan(user_request: String) -> PlanDraft {
    PlanDraft {
        description: user_request.clone(),
        background: user_request.clone(),
        call: Vec::new(),
        model: StepDraft {
            name: "Initial".to_owned(),
            kind: "model".to_owned(),
            description: user_request,
            input: String::new(),
        },
        future: Vec::new(),
        expected_result: String::new(),
    }
}

pub(super) fn call_output(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|step| format!("- {}: {}", step.signature().name, step.output()))
        .collect::<Vec<_>>()
        .join("\n")
}

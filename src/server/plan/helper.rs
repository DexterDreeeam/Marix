use marix_protocol::{PlanDraft, StepDraft};

pub(crate) fn initial_plan(user_request: String) -> PlanDraft {
    PlanDraft {
        description: user_request.clone(),
        background: user_request.clone(),
        call: Vec::new(),
        model: StepDraft {
            name: "Analysis".to_owned(),
            kind: "model".to_owned(),
            description: user_request,
            input: String::new(),
        },
        future: Vec::new(),
        expected_result: String::new(),
    }
}

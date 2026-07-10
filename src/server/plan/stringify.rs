use crate::plan::Plan;
use crate::step::Step;
use marix_protocol::{InvocationStepKind, ModelStepKind, StepKind, UserStepKind};

/// Read-only helper that renders a snapshot of plans into prompt text.
pub struct PlanStringify {
    plans: Vec<Plan>,
}

impl PlanStringify {
    pub fn new(plans: Vec<Plan>) -> Self {
        Self { plans }
    }

    pub fn current_plan_text(&self) -> String {
        self.plans
            .iter()
            .map(Self::plan_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn pending_intentions_text(&self) -> String {
        self.plans
            .iter()
            .flat_map(|plan| &plan.state.future)
            .map(|step| step.description().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// -- Private -- //

struct StepText {
    name: String,
    kind: &'static str,
    input: Option<String>,
}

impl PlanStringify {
    fn plan_text(plan: &Plan) -> String {
        let mut lines = vec![
            format!("description: {}", &plan.state.description),
            format!("background: {}", &plan.state.background),
            "call:".to_owned(),
        ];
        lines.extend(plan.state.call.iter().map(Self::step_text));
        lines.push("model:".to_owned());
        lines.push(Self::step_text_with_input(
            &plan.state.model,
            Self::model_input(plan),
        ));
        lines.push("future:".to_owned());
        lines.extend(plan.state.future.iter().map(Self::step_text));
        lines.push(format!(
            "expected_result: {}",
            &plan.state.expected_result
        ));
        lines.join("\n")
    }

    fn step_text(step: &Step) -> String {
        let text = Self::step_fields(step);
        Self::format_step_text(text, step.description())
    }

    fn step_text_with_input(step: &Step, input: Option<String>) -> String {
        let mut text = Self::step_fields(step);
        text.input = input;
        Self::format_step_text(text, step.description())
    }

    fn format_step_text(text: StepText, description: &str) -> String {
        let mut fields = format!(
            "- name: {}\n  kind: {}\n  description: {}",
            text.name,
            text.kind,
            description
        );
        if let Some(input) = text.input {
            fields.push_str(&format!("\n  input: {input}"));
        }
        fields
    }

    fn model_input(plan: &Plan) -> Option<String> {
        Some(
            plan.state
                .call
                .iter()
                .map(|step| step.signature().name.clone())
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    fn step_fields(step: &Step) -> StepText {
        match step.kind() {
            StepKind::Invocation(InvocationStepKind::Invocation(request)) => StepText {
                name: request.signature.name.clone(),
                kind: "tool",
                input: Some(request.input.content.clone()),
            },
            StepKind::Invocation(InvocationStepKind::Cancel) => StepText {
                name: "cancel".to_owned(),
                kind: "tool",
                input: Some(String::new()),
            },
            StepKind::Invocation(InvocationStepKind::Kill) => StepText {
                name: "kill".to_owned(),
                kind: "tool",
                input: Some(String::new()),
            },
            StepKind::Model(ModelStepKind::Initial) => StepText {
                name: "Initial".to_owned(),
                kind: "model",
                input: Some(String::new()),
            },
            StepKind::Model(ModelStepKind::Analysis) => StepText {
                name: "Analysis".to_owned(),
                kind: "model",
                input: Some(String::new()),
            },
            StepKind::Intent => StepText {
                name: "intent".to_owned(),
                kind: "intent",
                input: None,
            },
            StepKind::User(UserStepKind::Verdict) => StepText {
                name: "verdict".to_owned(),
                kind: "user",
                input: None,
            },
            StepKind::User(UserStepKind::Warrant) => StepText {
                name: "warrant".to_owned(),
                kind: "user",
                input: None,
            },
        }
    }
}

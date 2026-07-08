use crate::plan::PlanRecord;
use crate::step::Step;
use marix_protocol::{InvocationStepKind, ModelStepKind, StepKind, UserStepKind};

/// Read-only helper that renders a snapshot of plan records into prompt text.
pub struct PlanStringify {
    records: Vec<PlanRecord>,
}

impl PlanStringify {
    pub fn new(records: Vec<PlanRecord>) -> Self {
        Self { records }
    }

    pub fn current_plan_text(&self) -> String {
        self.records
            .iter()
            .map(Self::plan_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn pending_intentions_text(&self) -> String {
        self.records
            .iter()
            .flat_map(|record| record.plan.pending_steps.iter())
            .map(|step| step.description.clone())
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
    fn plan_text(record: &PlanRecord) -> String {
        let mut lines = vec![
            format!("description: {}", record.plan.description),
            "run_steps:".to_owned(),
        ];
        lines.extend(record.plan.run_steps.iter().map(Self::step_text));
        lines.push("pending_steps:".to_owned());
        lines.extend(record.plan.pending_steps.iter().map(Self::step_text));
        lines.push(format!("expected_result: {}", record.plan.expected_result));
        lines.join("\n")
    }

    fn step_text(step: &Step) -> String {
        let text = Self::step_fields(step);
        let mut fields = format!(
            "- name: {}\n  kind: {}\n  description: {}",
            text.name, text.kind, step.description
        );
        if let Some(input) = text.input {
            fields.push_str(&format!("\n  input: {input}"));
        }
        fields
    }

    fn step_fields(step: &Step) -> StepText {
        match &step.kind {
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
                input: Some("Initial".to_owned()),
            },
            StepKind::Model(ModelStepKind::Analysis) => StepText {
                name: "Analysis".to_owned(),
                kind: "model",
                input: Some("Analysis".to_owned()),
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

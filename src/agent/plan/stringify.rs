use crate::plan::PlanRecord;

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
            .map(|record| format!("{:?}", record.plan))
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

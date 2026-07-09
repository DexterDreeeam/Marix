use marix_common::Logger;
use marix_protocol::{InvocationStatus, PlanDraft, RelayStatus, StepDraft, TaskEvent};

use crate::step::Step;

impl Step {
    pub(super) fn on_invocation_update(&self, status: InvocationStatus) {
        match status {
            InvocationStatus::Created => {
                Logger::debug(format!(
                    "step {} invocation created (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            InvocationStatus::Started => {
                Logger::debug(format!(
                    "step {} invocation started (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            InvocationStatus::Processing { .. } => {
                Logger::debug(format!(
                    "step {} invocation update (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            InvocationStatus::Canceled => {
                self.fail_with_reason("invocation canceled".to_owned());
            }
            InvocationStatus::Succeed { .. } => {
                self.complete(String::new());
            }
            InvocationStatus::Failed => {
                self.fail_with_reason("invocation failed".to_owned());
            }
        }
    }

    pub(super) fn on_relay_update(&self, status: RelayStatus) {
        match status {
            RelayStatus::Created => {
                Logger::debug(format!(
                    "step {} relay created (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            RelayStatus::Started => {
                Logger::debug(format!(
                    "step {} relay started (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            RelayStatus::Processing { .. } => {
                Logger::debug(format!(
                    "step {} relay update (task {})",
                    self.signature(),
                    &self.signature().task,
                ));
            }
            RelayStatus::Canceled => {
                self.fail_with_reason("relay canceled".to_owned());
            }
            RelayStatus::Succeed { .. } => {
                self.complete(String::new());
            }
            RelayStatus::Failed => {
                self.fail_with_reason("relay failed".to_owned());
            }
        }
    }

    pub(super) fn on_invocation_complete(&self, content: &str) {
        let plan = PlanDraft {
            description: "Analyze completed execution output".to_owned(),
            run_steps: vec![StepDraft {
                name: "Analysis".to_owned(),
                kind: "model".to_owned(),
                description: content.to_owned(),
                input: "Analysis".to_owned(),
            }],
            pending_steps: Vec::new(),
            expected_result: "Execution analysis updates the remaining plan".to_owned(),
        };
        Self::send_task_event(&self.state, TaskEvent::PlanCreate(plan));
    }
}

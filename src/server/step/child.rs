use marix_common::Logger;
use marix_protocol::{
    InvocationEvent, InvocationRequest, InvocationSignature, InvocationStatus, InvocationStepKind,
    PlanDraft, RelayEvent, RelayRequest, RelaySignature, RelayStatus, StepDraft, StepKind,
    TaskEvent,
};

use crate::step::Step;

impl Step {
    pub(super) fn create_invocation(&self, request: InvocationRequest) {
        if request.signature.step != self.signature {
            let _ = Logger::warning(format!(
                "step {} rejected invocation {}: signature step mismatch",
                self.signature.id.0, request.signature.invocation_id.0
            ));
            return;
        }
        if let Some(invocation) = self.state.invocation_hub.create(&self.state, request) {
            if invocation
                .sender()
                .send(InvocationEvent::ExecutionCreate)
                .is_err()
            {
                let _ = Logger::warning(format!(
                    "invocation {} create failed: worker stopped",
                    invocation.signature.invocation_id.0
                ));
            }
        }
    }

    pub(super) fn dispatch_invocation(
        &self,
        signature: InvocationSignature,
        event: InvocationEvent,
    ) {
        let event_name = format!("{event:?}");
        match self.state.invocation_hub.with(&signature, |invocation| {
            invocation.sender().send(event).is_ok()
        }) {
            Some(true) => {}
            Some(false) => {
                let _ = Logger::warning(format!(
                    "invocation {} event {event_name} failed: worker stopped",
                    signature.invocation_id.0
                ));
            }
            None => {
                let _ = Logger::warning(format!(
                    "invocation {} event {event_name} not dispatched: invocation not found",
                    signature.invocation_id.0
                ));
            }
        }
    }

    pub(super) fn on_invocation_update(&self, status: InvocationStatus) {
        match status {
            InvocationStatus::Created => {
                let _ = Logger::debug(format!(
                    "step {} invocation created (task {})",
                    self.signature.id.0, self.signature.task.id.0
                ));
            }
            InvocationStatus::Started => {
                let _ = Logger::debug(format!(
                    "step {} invocation started (task {})",
                    self.signature.id.0, self.signature.task.id.0
                ));
            }
            InvocationStatus::Processing { .. } => {
                let _ = Logger::debug(format!(
                    "step {} invocation update (task {})",
                    self.signature.id.0, self.signature.task.id.0
                ));
            }
            InvocationStatus::Canceled => {
                self.fail_with_reason("invocation canceled".to_owned());
            }
            InvocationStatus::Succeed { .. } => {
                let content = self.invocation_content();
                self.complete(content);
            }
            InvocationStatus::Failed => {
                self.fail_with_reason("invocation failed".to_owned());
            }
        }
    }

    pub(super) fn create_relay(&self, request: RelayRequest) {
        if request.signature.step != self.signature {
            let _ = Logger::warning(format!(
                "step {} rejected relay {}: signature step mismatch",
                self.signature.id.0, request.signature.relay_id.0
            ));
            return;
        }
        let _ = self.state.relay_hub.create(&self.state, request);
    }

    pub(super) fn dispatch_relay(&self, signature: RelaySignature, event: RelayEvent) {
        let event_name = format!("{event:?}");
        match self
            .state
            .relay_hub
            .with(&signature, |relay| relay.sender().send(event).is_ok())
        {
            Some(true) => {}
            Some(false) => {
                let _ = Logger::warning(format!(
                    "relay {} event {event_name} failed: worker stopped",
                    signature.relay_id.0
                ));
            }
            None => {
                let _ = Logger::warning(format!(
                    "relay {} event {event_name} not dispatched: relay not found",
                    signature.relay_id.0
                ));
            }
        }
    }

    pub(super) fn on_relay_update(&self, status: RelayStatus) {
        match status {
            RelayStatus::Created => {
                let _ = Logger::debug(format!(
                    "step {} relay created (task {})",
                    self.signature.id.0, self.signature.task.id.0
                ));
            }
            RelayStatus::Started => {
                let _ = Logger::debug(format!(
                    "step {} relay started (task {})",
                    self.signature.id.0, self.signature.task.id.0
                ));
            }
            RelayStatus::Processing { .. } => {
                let _ = Logger::debug(format!(
                    "step {} relay update (task {})",
                    self.signature.id.0, self.signature.task.id.0
                ));
            }
            RelayStatus::Canceled => {
                self.fail_with_reason("relay canceled".to_owned());
            }
            RelayStatus::Succeed { .. } => {
                let content = self.relay_content();
                self.complete(content);
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

    fn invocation_content(&self) -> String {
        let StepKind::Invocation(InvocationStepKind::Invocation(request)) = &self.kind else {
            return String::new();
        };
        self.state
            .invocation_hub
            .content(&request.signature)
            .unwrap_or_default()
    }

    fn relay_content(&self) -> String {
        let StepKind::Invocation(InvocationStepKind::Invocation(_)) = &self.kind else {
            return String::new();
        };
        String::new()
    }
}

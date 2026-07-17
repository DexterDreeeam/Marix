use marix_common::{Logger, Runtime as RuntimeTrait};
use marix_protocol::{IntentEvent, PlanResult, PlanResultKind, SessionEvent, TaskEvent};

use super::PlanRuntime;

impl PlanRuntime {
    pub(super) fn finish(&self, kind: PlanResultKind, output: String) {
        if matches!(&kind, PlanResultKind::Failed) {
            Logger::error(format!("plan {} failed: {output}", &self.signature,));
        }
        let result = PlanResult { kind, output };
        RuntimeTrait::finish(self, result);
    }

    pub(super) fn cancel(&self, output: String) {
        if self.status().is_terminal() {
            return;
        }
        let intents = self
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        for signature in intents {
            let event = SessionEvent::Task(
                self.access.signature.clone(),
                TaskEvent::Intent(signature.clone(), IntentEvent::Cancel),
            );
            if self.access.session_tx.send(event).is_err() {
                Logger::warning(format!(
                    "plan {} child intent {signature} cancel failed: \
                     session stopped",
                    &self.signature,
                ));
            }
        }
        self.finish(PlanResultKind::Canceled, output);
    }
}

// -- Private -- //

impl PlanRuntime {
    pub(super) fn send_parent_event(&self, event: IntentEvent) {
        let intent = self.signature.intent.as_ref().clone();
        let event = SessionEvent::Task(intent.task.clone(), TaskEvent::Intent(intent, event));
        if self.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "plan {} event send failed: session stopped",
                &self.signature,
            ));
        }
    }
}

use marix_common::Logger;
use marix_protocol::{
    IntentEvent, PlanResult, PlanResultKind, PlanStatus, SessionEvent,
    TaskEvent,
};

use super::PlanRuntime;

impl PlanRuntime {
    pub(super) fn finish(&self, kind: PlanResultKind, output: String) {
        if matches!(&kind, PlanResultKind::Failed) {
            Logger::error(format!(
                "plan {} failed: {output}",
                &self.state.signature,
            ));
        }
        let result = PlanResult { kind, output };
        self.set_status(PlanStatus::Complete(result));
        self.close();
    }

    pub(super) fn cancel(&self, output: String) {
        if self.status().is_terminal() {
            return;
        }
        let intents = self
            .state
            .intents
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        for signature in intents {
            let event = SessionEvent::Task(
                self.state.access.signature.clone(),
                TaskEvent::Intent(
                    signature.clone(),
                    IntentEvent::Cancel,
                ),
            );
            if self.state.access.session_tx.send(event).is_err() {
                Logger::warning(format!(
                    "plan {} child intent {signature} cancel failed: \
                     session stopped",
                    &self.state.signature,
                ));
            }
        }
        self.finish(PlanResultKind::Canceled, output);
    }

    pub(super) fn status(&self) -> PlanStatus {
        self.state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(super) fn set_status(&self, status: PlanStatus) {
        let mut current = self
            .state
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current.is_terminal() {
            return;
        }
        let send_update = matches!(&status, PlanStatus::Complete(_));
        *current = status.clone();
        drop(current);
        if send_update {
            self.send_parent_event(IntentEvent::PlanUpdate(
                self.state.signature.clone(),
                status,
            ));
        }
    }
}

// -- Private -- //

impl PlanRuntime {
    fn send_parent_event(&self, event: IntentEvent) {
        let intent = self.state.signature.intent.as_ref().clone();
        let event = SessionEvent::Task(
            intent.task.clone(),
            TaskEvent::Intent(intent, event),
        );
        if self.state.access.session_tx.send(event).is_err() {
            Logger::warning(format!(
                "plan {} event send failed: session stopped",
                &self.state.signature,
            ));
        }
    }

    fn close(&self) {
        if self.close_tx.send(()).is_err() {
            Logger::warning(format!(
                "plan {} close signal failed",
                &self.state.signature,
            ));
        }
    }
}

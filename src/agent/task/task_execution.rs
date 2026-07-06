use std::sync::Arc;

use marix_protocol::{
    ExecutionEvent, ExecutionSignature, ExecutionStatus, ExecutionStepKind, ModelStepKind, Plan,
    PlanEvent, SessionEvent, StepDraft, StepEvent, StepKind, StepResult, StepSignature,
};

use crate::step::Step;
use crate::task::{Task, TaskState};

impl Task {
    pub(crate) fn on_execution_complete(
        state: Arc<TaskState>,
        _signature: &StepSignature,
        content: &str,
    ) {
        let plan = Plan {
            description: "Analyze completed execution output".to_owned(),
            run_steps: vec![StepDraft {
                kind: StepKind::Model(ModelStepKind::ExecutionAnalysis),
                description: content.to_owned(),
            }],
            pending_steps: Vec::new(),
            expected_result: "Execution analysis updates the remaining plan".to_owned(),
        };
        Step::send_plan_event(&state, PlanEvent::Trigger(plan));
    }

    pub(crate) fn run_execution_step(state: Arc<TaskState>, step: Step) {
        Step::send_step_event(&state, &step.signature, StepEvent::Started);
        let request = match &step.signature.kind {
            StepKind::Execution(ExecutionStepKind::Invocation(request)) => request.clone(),
            _ => {
                Step::send_step_event(
                    &state,
                    &step.signature,
                    StepEvent::Fail {
                        result: StepResult {
                            content: "execution step kind is not supported yet".to_owned(),
                        },
                    },
                );
                return;
            }
        };
        state
            .execution_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(request.signature.clone(), step.signature.clone());
        let _ = state.session_tx.send(SessionEvent::Execution(
            request.signature.clone(),
            ExecutionEvent::Evoke(request),
        ));
    }

    pub(super) fn route_execution_event(
        state: Arc<TaskState>,
        signature: ExecutionSignature,
        event: ExecutionEvent,
    ) -> bool {
        if !matches!(event, ExecutionEvent::Status(ExecutionStatus::Succeed)) {
            return true;
        }
        let Some(step_signature) = state
            .execution_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&signature)
            .cloned()
        else {
            return true;
        };
        let event = StepEvent::Complete {
            seq_count: 0,
            result: StepResult {
                content: String::new(),
            },
        };
        Step::send_step_event(state.as_ref(), &step_signature, event);
        true
    }
}

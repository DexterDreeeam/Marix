use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;

use marix_protocol::{
    ModelStepKind, Plan, SessionEvent, StepEvent, StepKind, StepResult, StepSignature,
};

use crate::model::{ModelRequest, ModelResponse};
use crate::prompt::{InitialPrompt, Prompt};
use crate::session::SessionContext;
use crate::step::Step;
use crate::task::{Task, TaskState};

impl Task {
    pub(super) fn route_step_event(
        state: Arc<TaskState>,
        signature: StepSignature,
        event: StepEvent,
    ) {
        if let StepEvent::Complete { result, .. } = event {
            state.steps.complete(signature.step_no);
            if let Ok(plan) = Plan::parse(&result.content) {
                let mut signatures = Vec::new();
                for (index, draft) in plan.ready_steps.iter().cloned().enumerate() {
                    let step = Step::new(StepSignature::new(
                        state.signature.clone(),
                        index,
                        draft.description,
                        draft.kind,
                    ));
                    signatures.push(step.signature.clone());
                    Self::run_step(Arc::clone(&state), step);
                }
                state
                    .plan_list
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .push((plan, signatures));
            }
        }
    }

    pub(super) fn run_step(state: Arc<TaskState>, mut step: Step) {
        if matches!(step.signature.kind, StepKind::Intent) {
            Self::send_step_event(
                &state,
                &step.signature,
                StepEvent::Fail {
                    result: StepResult {
                        content: "intent step is not executable".to_owned(),
                    },
                },
            );
            return;
        }
        if state.step_count.load(Ordering::Relaxed) >= 10 {
            Self::send_step_event(
                &state,
                &step.signature,
                StepEvent::Fail {
                    result: StepResult {
                        content: "task step limit exceeded".to_owned(),
                    },
                },
            );
            return;
        }
        let step_no = state.step_count.fetch_add(1, Ordering::Relaxed);
        step.signature.step_no = step_no;
        state.steps.insert_or_update(step_no, step.clone());
        thread::spawn(move || {
            if matches!(step.signature.kind, StepKind::Model(_)) {
                Self::execute_model_step(&state, step);
            } else {
                Self::send_step_event(
                    &state,
                    &step.signature,
                    StepEvent::Fail {
                        result: StepResult {
                            content: "task step kind is not supported yet".to_owned(),
                        },
                    },
                );
            }
        });
    }

    pub(super) fn initial_step(state: &TaskState) -> Step {
        Step::new(StepSignature::new(
            state.signature.clone(),
            0,
            state.signature.name.clone(),
            StepKind::Model(ModelStepKind::Initial),
        ))
    }

    pub(super) fn execute_model_step(state: &TaskState, step: Step) {
        Self::send_step_event(state, &step.signature, StepEvent::Started);
        let prompt = Self::model_step_prompt(state, &step);
        let update_count = Arc::clone(&step.update_count);
        let signature = step.signature.clone();
        let mut result = String::new();
        let request = ModelRequest { step, prompt };
        let responses = {
            let mut backend = state
                .model_backend
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            backend.request(request)
        };
        let responses = match responses {
            Ok(responses) => responses,
            Err(error) => {
                Self::send_step_event(
                    state,
                    &signature,
                    StepEvent::Fail {
                        result: StepResult {
                            content: error.to_string(),
                        },
                    },
                );
                return;
            }
        };
        for response in responses {
            match response {
                ModelResponse::Content(content) => {
                    let seq = update_count.fetch_add(1, Ordering::Relaxed);
                    result.push_str(&content);
                    Self::send_step_event(state, &signature, StepEvent::Update { seq, content });
                }
                ModelResponse::Failed(error) => {
                    Self::send_step_event(
                        state,
                        &signature,
                        StepEvent::Fail {
                            result: StepResult {
                                content: error.to_string(),
                            },
                        },
                    );
                    return;
                }
            }
        }
        let seq_count = update_count.load(Ordering::Relaxed);
        Self::send_step_event(
            state,
            &signature,
            StepEvent::Complete {
                seq_count,
                result: StepResult { content: result },
            },
        );
    }

    pub(super) fn send_step_event(state: &TaskState, signature: &StepSignature, event: StepEvent) {
        let _ = state
            .session_tx
            .send(SessionEvent::Step(signature.clone(), event));
    }

    pub(super) fn model_step_prompt(state: &TaskState, step: &Step) -> String {
        match step.signature.kind {
            StepKind::Model(ModelStepKind::Initial) => {
                let session_context = {
                    let context = state
                        .session_context
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    SessionContext {
                        system: context.system,
                        tasks: context.tasks.clone(),
                        tools: context.tools.clone(),
                    }
                };
                InitialPrompt::new(state.signature.name.clone(), session_context).prompt()
            }
            _ => state.signature.name.clone(),
        }
    }
}

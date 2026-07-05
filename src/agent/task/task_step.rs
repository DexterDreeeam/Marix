use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;

use marix_protocol::{
    ModelStepKind, Plan, PlanSignature, SessionEvent, StepDraft, StepEvent, StepKind, StepResult,
    StepSignature,
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
            if state.plan_queue.complete_step(&signature).is_some() {
                if let Ok(plan) = Plan::parse(&result.content) {
                    let plan_signature = Self::add_plan(&state, plan);
                    Self::run_plan(state, &plan_signature);
                }
            }
        }
    }

    pub(super) fn run_step(state: Arc<TaskState>, mut step: Step) -> Option<StepSignature> {
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
            return None;
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
            return None;
        }
        let step_no = state.step_count.fetch_add(1, Ordering::Relaxed);
        step.signature.step_no = step_no;
        let signature = step.signature.clone();
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
        Some(signature)
    }

    pub(super) fn generate_initial_plan(state: Arc<TaskState>) -> PlanSignature {
        let plan = Plan {
            description: state.user_request.clone(),
            run_steps: vec![StepDraft {
                kind: StepKind::Model(ModelStepKind::Initial),
                description: state.user_request.clone(),
            }],
            pending_steps: Vec::new(),
            expected_result: String::new(),
        };
        let plan_signature = Self::add_plan(&state, plan);
        Self::run_plan(state, &plan_signature);
        plan_signature
    }

    fn add_plan(state: &TaskState, plan: Plan) -> PlanSignature {
        let plan_signature = PlanSignature::new();
        let signatures = plan
            .run_steps
            .iter()
            .cloned()
            .enumerate()
            .map(|(step_no, draft)| {
                StepSignature::new(
                    state.signature.clone(),
                    step_no,
                    draft.description,
                    draft.kind,
                )
            })
            .collect();
        state
            .plan_queue
            .insert(plan_signature.clone(), plan, signatures)
            .unwrap_or_else(|error| panic!("failed to insert task plan: {error:?}"));
        plan_signature
    }

    fn run_plan(state: Arc<TaskState>, plan_signature: &PlanSignature) {
        let steps = match state.plan_queue.step_signatures(plan_signature) {
            Ok(steps) => steps,
            Err(_) => return,
        };
        for signature in steps {
            let step = Step::new(signature);
            Self::run_step(Arc::clone(&state), step);
        }
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
                InitialPrompt::new(state.user_request.clone(), session_context).prompt()
            }
            _ => state.user_request.clone(),
        }
    }
}

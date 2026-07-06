use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use marix_protocol::{
    Answer, ModelStepKind, Plan, PlanSignature, SessionEvent, StepDraft, StepEvent, StepKind,
    StepResult, StepSignature, StepStatus, TaskEvent, TaskResult, TaskStatus,
};

use crate::model::{ModelRequest, ModelResponse};
use crate::prompt::{ExecutionAnalysisPrompt, InitialPrompt, Prompt};
use crate::task::{Task, TaskState};

#[derive(Debug, Clone)]
pub struct Step {
    pub state: Arc<TaskState>,
    pub signature: StepSignature,
    pub status: StepStatus,
    pub update_count: Arc<AtomicUsize>,
}

impl Step {
    pub fn new(state: Arc<TaskState>, signature: StepSignature) -> Self {
        Self {
            state,
            signature,
            status: StepStatus::Prepare,
            update_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Step {
    pub(crate) fn route_step_event(
        state: Arc<TaskState>,
        signature: StepSignature,
        event: StepEvent,
    ) -> bool {
        if let StepEvent::Complete { result, .. } = event {
            return Self::on_step_complete(state, signature, result.content);
        }
        true
    }

    pub(crate) fn trigger_initial_plan(state: Arc<TaskState>) {
        let plan = Plan {
            description: state.user_request.clone(),
            run_steps: vec![StepDraft {
                kind: StepKind::Model(ModelStepKind::Initial),
                description: state.user_request.clone(),
            }],
            pending_steps: Vec::new(),
            expected_result: String::new(),
        };
        Self::run_plan(state, plan);
    }

    pub(crate) fn run_plan(state: Arc<TaskState>, plan: Plan) {
        let plan_signature = Self::add_plan(&state, plan);
        let steps = match state.plan_queue.step_signatures(&plan_signature) {
            Ok(steps) => steps,
            Err(_) => return,
        };
        for signature in steps {
            let step = Step::new(Arc::clone(&state), signature);
            step.run();
        }
    }

    pub(crate) fn send_step_event(state: &TaskState, signature: &StepSignature, event: StepEvent) {
        let _ = state
            .session_tx
            .send(SessionEvent::Step(signature.clone(), event));
    }
}

// -- Private -- //

impl Step {
    fn run(self) -> Option<StepSignature> {
        if matches!(self.signature.kind, StepKind::Intent) {
            Self::send_step_event(
                &self.state,
                &self.signature,
                StepEvent::Fail {
                    result: StepResult {
                        content: "intent step is not executable".to_owned(),
                    },
                },
            );
            return None;
        }
        if self.signature.step_no >= 10 {
            Self::send_step_event(
                &self.state,
                &self.signature,
                StepEvent::Fail {
                    result: StepResult {
                        content: "task step limit exceeded".to_owned(),
                    },
                },
            );
            return None;
        }
        let step_no = self.signature.step_no;
        let signature = self.signature.clone();
        self.state.steps.insert_or_update(step_no, self.clone());
        match &self.signature.kind {
            StepKind::Model(_) => self.run_model(),
            StepKind::Execution(_) => Task::run_execution_step(Arc::clone(&self.state), self),
            StepKind::Intent | StepKind::User(_) => Self::send_step_event(
                &self.state,
                &self.signature,
                StepEvent::Fail {
                    result: StepResult {
                        content: "task step kind is not supported yet".to_owned(),
                    },
                },
            ),
        }
        Some(signature)
    }

    fn run_model(self) {
        Self::send_step_event(&self.state, &self.signature, StepEvent::Started);
        thread::spawn(move || {
            let state = Arc::clone(&self.state);
            let prompt = self.model_prompt();
            let update_count = Arc::clone(&self.update_count);
            let signature = self.signature.clone();
            let mut result = String::new();
            let request = ModelRequest { step: self, prompt };
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
                        &state,
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
                        Self::send_step_event(
                            &state,
                            &signature,
                            StepEvent::Update { seq, content },
                        );
                    }
                    ModelResponse::Failed(error) => {
                        Self::send_step_event(
                            &state,
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
            let event = StepEvent::Complete {
                seq_count,
                result: StepResult { content: result },
            };
            Self::send_step_event(&state, &signature, event);
        });
    }

    fn model_prompt(&self) -> String {
        match self.signature.kind {
            StepKind::Model(ModelStepKind::Initial) => {
                let session_context = self
                    .state
                    .session_context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .snapshot();
                InitialPrompt::new(self.state.user_request.clone(), session_context).prompt()
            }
            StepKind::Model(ModelStepKind::ExecutionAnalysis) => {
                let session_context = self
                    .state
                    .session_context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .snapshot();
                ExecutionAnalysisPrompt::new(
                    self.state.user_request.clone(),
                    self.signature.description.clone(),
                    self.state.plan_queue.current_plan_text(),
                    self.state.plan_queue.pending_intentions_text(),
                    session_context,
                )
                .prompt()
            }
            _ => self.state.user_request.clone(),
        }
    }

    fn on_step_complete(state: Arc<TaskState>, signature: StepSignature, content: String) -> bool {
        state.steps.complete(signature.step_no);
        let Some(_) = state.plan_queue.complete_step(&signature) else {
            return true;
        };
        match &signature.kind {
            StepKind::Model(_) => Self::on_model_complete(state, &content),
            StepKind::Execution(_) => {
                Task::on_execution_complete(state, &signature, &content);
                true
            }
            StepKind::Intent | StepKind::User(_) => true,
        }
    }

    fn on_model_complete(state: Arc<TaskState>, content: &str) -> bool {
        if let Ok(answer) = Answer::parse(content) {
            let event = SessionEvent::Task(
                state.signature.clone(),
                TaskEvent::Status(TaskStatus::Succeed(TaskResult {
                    content: answer.answer,
                })),
            );
            let _ = state.session_tx.send(event);
            return false;
        }
        if let Ok(plan) = Plan::parse(content) {
            Self::run_plan(state, plan);
        }
        true
    }

    fn add_plan(state: &TaskState, plan: Plan) -> PlanSignature {
        let plan_signature = PlanSignature::new(state.signature.clone());
        // Step numbers are derived from the current number of steps in the queue.
        // add_plan builds all of a plan's signatures before any step is inserted,
        // so the enumerate offset keeps them unique within this batch; run_plan is
        // processed serially, so steps.size() is stable between plans.
        let base_step_no = state.steps.size();
        let signatures = plan
            .run_steps
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, draft)| {
                StepSignature::new(
                    state.signature.clone(),
                    base_step_no + index,
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
}

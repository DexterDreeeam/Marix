use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use marix_protocol::{
    Answer, ModelStepKind, Plan, PlanEvent, PlanSignature, SessionEvent, StepDraft, StepEvent,
    StepKind, StepResult, StepSignature, TaskEvent, TaskResult, TaskStatus,
};

use crate::model::{ModelRequest, ModelResponse};
use crate::prompt::{ExecutionAnalysisPrompt, InitialPrompt, Prompt};
use crate::task::{Task, TaskState};

#[derive(Debug, Clone)]
pub struct Step {
    pub state: Arc<TaskState>,
    pub signature: StepSignature,
    pub update_count: Arc<AtomicUsize>,
}

impl Step {
    pub fn new(state: Arc<TaskState>, signature: StepSignature) -> Self {
        Self {
            state,
            signature,
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
        match event {
            StepEvent::Trigger => {
                Step::new(state, signature).run();
                true
            }
            StepEvent::Complete { result, .. } => {
                Self::on_step_complete(state, signature, result.content)
            }
            _ => true,
        }
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
        Self::send_plan_event(&state, PlanEvent::Trigger(plan));
    }

    pub(crate) fn send_step_event(state: &TaskState, signature: &StepSignature, event: StepEvent) {
        let _ = state
            .session_tx
            .send(SessionEvent::Step(signature.clone(), event));
    }

    pub(crate) fn send_plan_event(state: &TaskState, event: PlanEvent) {
        let signature = PlanSignature::new(state.signature.clone());
        let _ = state
            .session_tx
            .send(SessionEvent::Plan(signature, event));
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
                let plan_stringify = self.state.plan_hub.stringify();
                ExecutionAnalysisPrompt::new(
                    self.state.user_request.clone(),
                    self.signature.description.clone(),
                    plan_stringify.current_plan_text(),
                    plan_stringify.pending_intentions_text(),
                    session_context,
                )
                .prompt()
            }
            _ => self.state.user_request.clone(),
        }
    }

    fn on_step_complete(state: Arc<TaskState>, signature: StepSignature, content: String) -> bool {
        state.steps.complete(signature.step_no);
        let Some(_) = state.plan_hub.complete_step(&signature) else {
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
            Self::send_plan_event(&state, PlanEvent::Trigger(plan));
        }
        true
    }
}

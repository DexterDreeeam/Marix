use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use marix_common::Logger;
use marix_common::external::*;
use marix_protocol::{
    Answer, ExecutionRequest, ExecutionSignature, ExecutionStepKind, ModelStepKind, PlanDraft,
    PlanEvent, PlanSignature, SessionEvent, StepDraft, StepEvent, StepKind, StepResult,
    StepSignature, TaskEvent, TaskResult, TaskStatus, ToolInputSchema,
};

use crate::model::{ModelRequest, ModelResponse};
use crate::plan::PlanError;
use crate::prompt::{AnalysisPrompt, InitialPrompt, Prompt};
use crate::task::TaskState;

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

    pub(crate) fn from_draft(state: &Arc<TaskState>, draft: StepDraft) -> Result<Self, PlanError> {
        let kind = Self::step_kind(state, &draft)?;
        let signature = StepSignature::new(state.signature.clone(), draft.description, kind);
        Ok(Self::new(Arc::clone(state), signature))
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
                Self::on_complete(state, signature, result.content)
            }
            _ => true,
        }
    }

    pub(crate) fn trigger_initial_plan(state: Arc<TaskState>) {
        let plan = PlanDraft {
            description: state.user_request.clone(),
            run_steps: vec![StepDraft {
                name: "Initial".to_owned(),
                kind: "model".to_owned(),
                description: state.user_request.clone(),
                input: "Initial".to_owned(),
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
        let _ = state.session_tx.send(SessionEvent::Plan(signature, event));
    }
}

// -- Private -- //

impl Step {
    fn step_kind(state: &TaskState, draft: &StepDraft) -> Result<StepKind, PlanError> {
        match draft.kind.trim() {
            "tool" => Ok(StepKind::Execution(ExecutionStepKind::Invocation(
                ExecutionRequest {
                    signature: ExecutionSignature::new(state.signature.clone(), draft.name.clone()),
                    input: ToolInputSchema {
                        content: draft.input.clone(),
                    },
                },
            ))),
            "intent" => Ok(StepKind::Intent),
            "model" => Ok(StepKind::Model(Self::model_step_kind(draft)?)),
            kind => Err(PlanError::InvalidStepKind(kind.to_owned())),
        }
    }

    fn model_step_kind(draft: &StepDraft) -> Result<ModelStepKind, PlanError> {
        Self::parse_model_step_name(&draft.name)
            .or_else(|| Self::parse_model_step_name(Self::input_model_name(&draft.input)))
            .ok_or_else(|| PlanError::InvalidModelStep {
                name: draft.name.clone(),
                input: draft.input.clone(),
            })
    }

    fn parse_model_step_name(name: &str) -> Option<ModelStepKind> {
        match name.trim() {
            "Initial" | "initial" => Some(ModelStepKind::Initial),
            "Analysis" | "analysis" => Some(ModelStepKind::Analysis),
            _ => None,
        }
    }

    fn input_model_name(input: &str) -> &str {
        input.split(',').next().unwrap_or_default().trim()
    }

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
        if self.state.steps.size() >= 10 {
            let _ = Logger::warning(format!(
                "step {} exceeds limit (task {})",
                self.signature.id.0, self.signature.task.id.0
            ));
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
        let step_id = self.signature.id.clone();
        let signature = self.signature.clone();
        self.state.steps.insert_or_update(step_id, self.clone());
        match &self.signature.kind {
            StepKind::Model(_) => self.run_model(),
            StepKind::Execution(_) => {
                let state = Arc::clone(&self.state);
                state.execution_hub.run_execution_step(&state, self);
            }
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
            let _ = Logger::debug(format!(
                "model step {} request (task {})",
                signature.id.0, signature.task.id.0
            ));
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
                    let _ = Logger::error(format!(
                        "model step {} request failed: {error}",
                        signature.id.0
                    ));
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
                        let _ = Logger::error(format!(
                            "model step {} stream failed: {error}",
                            signature.id.0
                        ));
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
            let _ = Logger::debug(format!(
                "model step {} completed ({seq_count} chunks)",
                signature.id.0
            ));
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
            StepKind::Model(ModelStepKind::Analysis) => {
                let session_context = self
                    .state
                    .session_context
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .snapshot();
                let plan_stringify = self.state.plan_hub.stringify();
                AnalysisPrompt::new(
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

    fn on_complete(state: Arc<TaskState>, signature: StepSignature, content: String) -> bool {
        state.steps.complete(signature.id.clone());
        let Some(_) = state.plan_hub.complete_step(&signature) else {
            return true;
        };
        match &signature.kind {
            StepKind::Model(_) => Self::on_model_complete(state, &content),
            StepKind::Execution(_) => {
                Self::on_execution_complete(&state, &content);
                true
            }
            StepKind::Intent | StepKind::User(_) => true,
        }
    }

    fn on_execution_complete(state: &TaskState, content: &str) {
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
        Self::send_plan_event(state, PlanEvent::Trigger(plan));
    }

    fn on_model_complete(state: Arc<TaskState>, content: &str) -> bool {
        let value = match serde_json::from_str::<serde_json::Value>(content) {
            Ok(value) => value,
            Err(error) => {
                let _ = Logger::warning(format!("model output is not valid JSON: {error}"));
                return true;
            }
        };
        let Some(object) = value.as_object() else {
            let _ = Logger::warning("model output JSON is not an object");
            return true;
        };
        if object.get("answer").is_some() {
            match serde_json::from_str::<Answer>(content) {
                Ok(answer) => {
                    let _ = Logger::log(format!(
                        "task {} produced final answer",
                        state.signature.id.0
                    ));
                    let event = SessionEvent::Task(
                        state.signature.clone(),
                        TaskEvent::Status(TaskStatus::Succeed(TaskResult {
                            content: answer.answer,
                        })),
                    );
                    let _ = state.session_tx.send(event);
                    return false;
                }
                Err(error) => {
                    let _ = Logger::warning(format!("model output is not a valid answer: {error}"));
                    return true;
                }
            }
        }
        match serde_json::from_str::<PlanDraft>(content) {
            Ok(plan) => {
                let _ = Logger::debug(format!(
                    "model produced a plan with {} run step(s)",
                    plan.run_steps.len()
                ));
                Self::send_plan_event(&state, PlanEvent::Trigger(plan));
            }
            Err(error) => {
                let _ = Logger::warning(format!("model output is not a valid plan: {error}"));
            }
        }
        true
    }
}

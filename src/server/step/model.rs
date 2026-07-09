use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;

use marix_common::Logger;
use marix_common::external::*;
use marix_protocol::{
    Answer, ModelStepKind, PlanDraft, StepKind, TaskEvent, TaskResult, TaskStatus,
};

use crate::model::{ModelRequest, ModelResponse};
use crate::prompt::{AnalysisPrompt, InitialPrompt, Prompt};
use crate::step::Step;

impl Step {
    pub(super) fn run_model(self) {
        Logger::debug(format!(
            "model step {} started (task {})",
            self.signature.id.0, self.signature.task.id.0
        ));
        thread::spawn(move || {
            let state = Arc::clone(&self.state);
            let prompt = self.model_prompt();
            let update_count = Arc::clone(&self.update_count);
            let signature = self.signature.clone();
            Logger::debug(format!(
                "model step {} request (task {})",
                signature.id.0, signature.task.id.0
            ));
            let mut result = String::new();
            let request = ModelRequest {
                step: self.clone(),
                prompt,
            };
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
                    let reason = error.to_string();
                    Logger::error(format!(
                        "model step {} request failed: {reason}",
                        signature.id.0
                    ));
                    self.fail_with_reason(reason);
                    return;
                }
            };
            for response in responses {
                match response {
                    ModelResponse::Content(content) => {
                        let seq = update_count.fetch_add(1, Ordering::Relaxed);
                        result.push_str(&content);
                        Logger::debug(format!(
                            "model step {} update {seq} (task {})",
                            signature.id.0, signature.task.id.0
                        ));
                    }
                    ModelResponse::Failed(error) => {
                        let reason = error.to_string();
                        Logger::error(format!(
                            "model step {} stream failed: {reason}",
                            signature.id.0
                        ));
                        self.fail_with_reason(reason);
                        return;
                    }
                }
            }
            let seq_count = update_count.load(Ordering::Relaxed);
            Logger::debug(format!(
                "model step {} completed ({seq_count} chunks)",
                signature.id.0
            ));
            self.complete(result);
        });
    }

    pub(super) fn on_model_complete(&self, content: &str) {
        let value = match serde_json::from_str::<serde_json::Value>(content) {
            Ok(value) => value,
            Err(error) => {
                let reason = format!("model output is not valid JSON: {error}");
                Logger::warning(reason.clone());
                Self::send_task_update(&self.state, TaskStatus::Failed { reason });
                return;
            }
        };
        let Some(object) = value.as_object() else {
            let reason = "model output JSON is not an object".to_owned();
            Logger::warning(reason.clone());
            Self::send_task_update(&self.state, TaskStatus::Failed { reason });
            return;
        };
        if object.get("answer").is_some() {
            match serde_json::from_str::<Answer>(content) {
                Ok(answer) => {
                    Logger::log(format!(
                        "task {} produced final answer",
                        self.state.signature.id.0
                    ));
                    Self::send_task_update(
                        &self.state,
                        TaskStatus::Succeed(TaskResult {
                            content: answer.answer,
                        }),
                    );
                    return;
                }
                Err(error) => {
                    let reason = format!("model output is not a valid answer: {error}");
                    Logger::warning(reason.clone());
                    Self::send_task_update(&self.state, TaskStatus::Failed { reason });
                    return;
                }
            }
        }
        match serde_json::from_str::<PlanDraft>(content) {
            Ok(plan) => {
                Logger::debug(format!(
                    "model produced a plan with {} run step(s)",
                    plan.run_steps.len()
                ));
                Self::send_task_event(&self.state, TaskEvent::PlanCreate(plan));
            }
            Err(error) => {
                let reason = format!("model output is not a valid plan: {error}");
                Logger::warning(reason.clone());
                Self::send_task_update(&self.state, TaskStatus::Failed { reason });
            }
        }
    }

    fn model_prompt(&self) -> String {
        match &self.kind {
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
                    self.description.clone(),
                    plan_stringify.current_plan_text(),
                    plan_stringify.pending_intentions_text(),
                    session_context,
                )
                .prompt()
            }
            _ => self.state.user_request.clone(),
        }
    }
}

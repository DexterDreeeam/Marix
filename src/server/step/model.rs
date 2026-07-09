use marix_common::Logger;
use marix_common::external::*;
use marix_protocol::{Answer, PlanDraft, TaskEvent, TaskResult, TaskStatus};

use crate::step::Step;

impl Step {
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
                        &self.state.access.signature,
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
}

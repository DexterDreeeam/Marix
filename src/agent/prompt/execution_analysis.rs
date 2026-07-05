use std::fs;
use std::path::Path;

use marix_common::Config;

use crate::prompt::{Prompt, render_template};
use crate::session::SessionContext;

pub struct ExecutionAnalysisPrompt {
    pub request_brief: String,
    pub execution_output: String,
    pub current_plan: String,
    pub pending_intentions: String,
    pub session_context: SessionContext,
}

impl ExecutionAnalysisPrompt {
    pub fn new(
        request_brief: String,
        execution_output: String,
        current_plan: String,
        pending_intentions: String,
        session_context: SessionContext,
    ) -> Self {
        Self {
            request_brief,
            execution_output,
            current_plan,
            pending_intentions,
            session_context,
        }
    }
}

impl Prompt for ExecutionAnalysisPrompt {
    fn load(name: &str) -> String {
        let config =
            Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
        let path = Path::new(&config.runtime.marix_path)
            .join("src")
            .join("prompt")
            .join("step")
            .join(format!("{name}.prompt"));
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to load prompt {}: {error}", path.display()))
    }

    fn prompt(&self) -> String {
        render_template(
            &Self::load("ExecutionAnalysis"),
            &[
                ("RequestBrief", self.request_brief.clone()),
                ("ExecutionOutput", self.execution_output.clone()),
                ("CurrentPlan", self.current_plan.clone()),
                ("PendingIntentions", self.pending_intentions.clone()),
                ("Tools", self.tools_text()),
            ],
        )
    }
}

// -- Private -- //

impl ExecutionAnalysisPrompt {
    fn tools_text(&self) -> String {
        self.session_context
            .tools
            .iter()
            .map(|tool| {
                format!(
                    "- {}: {}\n  input: {}\n  output: {}",
                    tool.name,
                    tool.description,
                    tool.schema.input.content,
                    tool.schema.output.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

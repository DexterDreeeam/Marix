use std::fs;
use std::path::Path;

use marix_common::Config;
use marix_common::external::*;

use crate::prompt::{Prompt, render_template};
use crate::session::SessionContext;

pub struct AnalysisPrompt {
    pub request: String,
    pub background: String,
    pub call_output: String,
    pub session_context: SessionContext,
}

impl AnalysisPrompt {
    pub fn new(
        request: String,
        background: String,
        call_output: String,
        session_context: SessionContext,
    ) -> Self {
        Self {
            request,
            background,
            call_output,
            session_context,
        }
    }
}

impl Prompt for AnalysisPrompt {
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
            &Self::load("Analysis"),
            &[
                ("Tools", self.tools_text()),
                ("Request", self.request.clone()),
                ("Background", self.background.clone()),
                ("CallOutput", self.call_output.clone()),
            ],
        )
    }
}

// -- Private -- //

impl AnalysisPrompt {
    fn tools_text(&self) -> String {
        serde_json::to_string(&self.session_context.tools)
            .unwrap_or_else(|error| panic!("failed to serialize prompt tools: {error}"))
    }
}

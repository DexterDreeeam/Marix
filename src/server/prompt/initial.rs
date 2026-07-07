use std::fs;
use std::path::Path;

use marix_common::Config;

use crate::prompt::{Prompt, render_template};
use crate::session::SessionContext;

pub struct InitialPrompt {
    pub request: String,
    pub session_context: SessionContext,
}

impl InitialPrompt {
    pub fn new(request: String, session_context: SessionContext) -> Self {
        Self {
            request,
            session_context,
        }
    }
}

impl Prompt for InitialPrompt {
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
        let no_preview_task = self.session_context.tasks.is_empty().to_string();
        render_template(
            &Self::load("Initial"),
            &[
                ("Request", self.request.clone()),
                ("System", self.system_text()),
                ("Context", self.session_context_text()),
                ("NoPreviewTask", no_preview_task),
                ("Tools", self.tools_text()),
            ],
        )
    }
}

// -- Private -- //

impl InitialPrompt {
    fn system_text(&self) -> String {
        match self.session_context.system {
            Some(system) => format!(
                "Platform: {:?}\nArchitecture: {:?}",
                system.platform, system.arch
            ),
            None => "Platform: unknown\nArchitecture: unknown".to_owned(),
        }
    }

    fn session_context_text(&self) -> String {
        self.session_context
            .tasks
            .iter()
            .map(|task| {
                format!(
                    "- request: {}\n  result: {}",
                    task.request.content, task.result.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

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

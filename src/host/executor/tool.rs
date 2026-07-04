use std::path::{Path, PathBuf};
use std::process::Command;

use marix_common::ToolPreview;

#[derive(Clone)]
pub struct Tool {
    path: PathBuf,
    preview: ToolPreview,
}

impl Tool {
    pub fn load(path: &Path) -> Option<Self> {
        let output = Command::new(path).arg("--preview").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8(output.stdout).ok()?;
        let preview = ToolPreview::from_json(stdout.trim()).ok()?;
        Some(Self {
            path: path.to_path_buf(),
            preview,
        })
    }

    pub fn name(&self) -> String {
        self.preview.name.clone()
    }

    pub fn description(&self) -> String {
        self.preview.description.clone()
    }

    pub fn input_schema(&self) -> String {
        self.preview.schema.input.content.clone()
    }

    pub fn output_schema(&self) -> String {
        self.preview.schema.output.content.clone()
    }

    pub fn preview(&self) -> ToolPreview {
        self.preview.clone()
    }

    pub fn execute(&self, input: &str) -> String {
        let output = Command::new(&self.path)
            .arg("--run")
            .arg(input)
            .output()
            .unwrap_or_else(|error| panic!("failed to run tool {}: {error}", self.path.display()));
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}

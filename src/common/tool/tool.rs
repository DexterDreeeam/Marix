use std::path::{Path, PathBuf};

use crate::external::*;
use crate::tool::ToolSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPreview {
    pub name: String,
    pub description: String,
    pub schema: ToolSchema,
}

impl ToolPreview {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

pub struct Tool {
    path: PathBuf,
}

impl Tool {
    pub fn load(path: &Path) -> Self {
        panic!("not implemented")
    }

    pub fn name(&self) -> String {
        panic!("not implemented")
    }

    pub fn description(&self) -> String {
        panic!("not implemented")
    }

    pub fn input_schema(&self) -> String {
        panic!("not implemented")
    }

    pub fn output_schema(&self) -> String {
        panic!("not implemented")
    }

    pub fn preview(&self) -> ToolPreview {
        panic!("not implemented")
    }

    pub fn execute(&self, input: &str) -> String {
        panic!("not implemented")
    }
}

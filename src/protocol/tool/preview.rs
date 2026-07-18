use marix_common::System;

use crate::external::*;
use crate::{ToolCategory, ToolInputSchema};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPreview {
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    pub system: System,
    pub input: ToolInputSchema,
}

impl ToolPreview {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

use marix_common::external::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToolCallSummary {
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) continuation_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptProfile {
    ToolMandatory,
    ToolCallSummary,
}

impl PromptProfile {
    pub(crate) fn compose_messages(
        &self,
        system: &str,
        prompts: &[String],
    ) -> Vec<serde_json::Value> {
        let mut messages = Vec::with_capacity(prompts.len() + 1);
        messages.push(serde_json::json!({
            "role": "system",
            "content": system
        }));
        if let Some((workflow_policy, remaining_prompts)) =
            prompts.split_first()
        {
            messages.push(serde_json::json!({
                "role": "system",
                "content": workflow_policy
            }));
            messages.extend(remaining_prompts.iter().map(|prompt| {
                serde_json::json!({
                    "role": "user",
                    "content": prompt
                })
            }));
        }
        messages
    }

    pub(crate) fn tool_choice(&self) -> serde_json::Value {
        match self {
            Self::ToolMandatory => serde_json::json!("required"),
            Self::ToolCallSummary => serde_json::json!("none"),
        }
    }

    pub(crate) fn expects_tool_calls(&self) -> bool {
        matches!(self, Self::ToolMandatory)
    }

    pub(crate) fn requires_json_output(&self) -> bool {
        matches!(self, Self::ToolCallSummary)
    }
}

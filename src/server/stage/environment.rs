use std::collections::BTreeSet;

use marix_common::external::*;
use marix_common::{Arch, Platform, System};
use marix_protocol::ToolPreview;

use crate::prompt::Prompt;
use crate::task::TaskAccess;

pub(super) fn request_environment(
    access: &TaskAccess,
) -> Result<(String, Vec<ToolPreview>), String> {
    let session_context = access.session_context()?;
    let (system, tools) = {
        let context = session_context
            .lock()
            .unwrap();
        let system = context.system.ok_or_else(|| {
            "current execution environment is unavailable".to_owned()
        })?;
        (system, context.tools.clone())
    };
    Ok((
        system_prompt(system)?,
        ordinary_tools(tools)?,
    ))
}

// -- Private -- //

fn system_prompt(system: System) -> Result<String, String> {
    let mut prompt = Prompt::load("System");
    for parameter in prompt.parameters() {
        let value = match parameter.name.as_str() {
            "system" => system_text(system),
            other => {
                return Err(format!(
                    "unsupported System prompt parameter `{other}`"
                ));
            }
        };
        prompt.inject(parameter.name, value);
    }
    prompt.prompt().map_err(|error| {
        format!("failed to render System prompt: {error}")
    })
}

fn system_text(system: System) -> String {
    let platform = match system.platform {
        Platform::All => "all supported operating systems",
        Platform::Win => "Windows",
        Platform::Ubuntu => "Ubuntu",
    };
    let arch = match system.arch {
        Arch::All => "all supported 64-bit architectures",
        Arch::Amd => "amd64",
        Arch::Arm => "arm",
    };
    format!("{platform} on {arch}")
}

fn ordinary_tools(
    execution_tools: Vec<ToolPreview>,
) -> Result<Vec<ToolPreview>, String> {
    let mut names = BTreeSet::new();
    let mut tools = Vec::with_capacity(execution_tools.len());
    for mut tool in execution_tools {
        if !names.insert(tool.name.clone()) {
            return Err(format!(
                "cannot send duplicate execution tool name `{}`",
                tool.name,
            ));
        }
        if let Ok(mut schema) =
            serde_json::from_str::<serde_json::Value>(&tool.input)
        {
            if let Some(properties) = schema
                .get_mut("properties")
                .and_then(serde_json::Value::as_object_mut)
            {
                properties.insert(
                    "purpose".to_owned(),
                    serde_json::json!({
                        "type": "string",
                        "description": "A short summary of what this tool invocation is doing and why."
                    }),
                );
            }
            if let Some(required) = schema
                .get_mut("required")
                .and_then(serde_json::Value::as_array_mut)
            {
                required.push(serde_json::json!("purpose"));
            }
            tool.input = serde_json::to_string(&schema)
                .unwrap_or_else(|_| tool.input);
        }
        tools.push(tool);
    }
    Ok(tools)
}

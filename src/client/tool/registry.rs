use super::category::{CategoryPreview, TOOL_CATEGORY_LIST};
use super::error::ToolError;
use super::native::native_tools;
use super::tool::{Tool, ToolPreview, ToolType};
use crate::common::config::{Config, Platform};

pub struct DefaultPreview {
    pub primary_tool_previews: Vec<ToolPreview>,
    pub category_previews: Vec<CategoryPreview>,
}

/// Collection of available tools, keyed by name. Builtins are registered at
/// startup; user tools are registered at runtime.
pub struct ToolRegistry {
    platform: Platform,
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(config: &Config) -> Self {
        let mut registry = Self {
            platform: config.platform,
            tools: Vec::new(),
        };
        for tool in native_tools() {
            if registry.supports_tool(tool.as_ref()) {
                registry.register(tool).unwrap_or_else(|error| {
                    panic!("failed to register native tool: {error:?}");
                });
            }
        }
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) -> Result<(), ToolError> {
        let name = tool.name();
        if !self.supports_tool(tool.as_ref()) {
            return Err(ToolError::Denied(format!(
                "tool {name} does not support platform {:?}",
                self.platform
            )));
        }
        if self
            .tools
            .iter()
            .any(|registered| registered.name() == name)
        {
            return Err(ToolError::DuplicateName(name.to_string()));
        }
        self.tools.push(tool);
        Ok(())
    }

    pub fn default_preview(&self) -> DefaultPreview {
        DefaultPreview {
            primary_tool_previews: self.preview_by_type(ToolType::Primary),
            category_previews: TOOL_CATEGORY_LIST.to_vec(),
        }
    }

    pub fn tool_preview(&self) -> Vec<ToolPreview> {
        self.tools.iter().map(|tool| tool.preview()).collect()
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|tool| tool.name() == name)
            .map(|tool| tool.as_ref())
    }
}

// -- Private -- //

impl ToolRegistry {
    fn preview_by_type(&self, tool_type: ToolType) -> Vec<ToolPreview> {
        self.tools
            .iter()
            .filter(|tool| tool.tool_type() == tool_type)
            .map(|tool| tool.preview())
            .collect()
    }

    fn supports_tool(&self, tool: &dyn Tool) -> bool {
        match tool.platform() {
            Platform::All => true,
            platform => platform == self.platform,
        }
    }
}

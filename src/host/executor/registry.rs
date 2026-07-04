use crate::executor::Tool;
use marix_common::Config;
use marix_protocol::ToolPreview;

/// Error produced while registering a tool into a [`ToolRegistry`].
pub enum RegistryError {
    /// A tool with the same name is already registered.
    DuplicateName(String),
}

/// Collection of available host tools keyed by name.
///
/// Tools are loaded from the tool directory declared in the global
/// configuration at startup; additional tools can be registered at runtime
/// through [`ToolRegistry::register`].
pub struct ToolRegistry {
    tools: Vec<Tool>,
}

impl ToolRegistry {
    /// Build a registry populated with every tool that loads successfully from
    /// the tool directory declared in the global configuration.
    pub fn new() -> Self {
        let mut registry = Self { tools: Vec::new() };
        let config =
            Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
        let Ok(entries) = std::fs::read_dir(&config.tool.directory) else {
            return registry;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(tool) = Tool::load(&path) {
                // Skip tools whose name collides with an already registered one.
                let _ = registry.register(tool);
            }
        }
        registry
    }

    /// Register a tool handle, rejecting a tool whose name is already present.
    pub fn register(&mut self, tool: Tool) -> Result<(), RegistryError> {
        let name = tool.name();
        if self
            .tools
            .iter()
            .any(|registered| registered.name() == name)
        {
            return Err(RegistryError::DuplicateName(name));
        }
        self.tools.push(tool);
        Ok(())
    }

    /// Look up a registered tool by name.
    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|tool| tool.name() == name)
    }

    /// Advertise a preview for every registered tool.
    pub fn preview(&self) -> Vec<ToolPreview> {
        self.tools.iter().map(|tool| tool.preview()).collect()
    }
}

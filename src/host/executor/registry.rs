use crate::executor::Tool;
use marix_common::{Config, Logger, System};
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
        let host_system = System::new();
        let config =
            Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
        let Ok(entries) = std::fs::read_dir(&config.tool.directory) else {
            Logger::warning(format!(
                "tool directory unavailable: {}",
                config.tool.directory
            ));
            return registry;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(tool) = Tool::load(&path) {
                // Skip tools whose name collides with an already registered one.
                let _ = registry.register_for_system(tool, &host_system);
            }
        }
        let mut tool_names = registry
            .tools
            .iter()
            .map(|tool| tool.name())
            .collect::<Vec<_>>();
        tool_names.sort_unstable();
        if tool_names.is_empty() {
            Logger::log("host loaded 0 tools");
        } else {
            Logger::log(format!(
                "host loaded {} tools: {}",
                tool_names.len(),
                tool_names.join(", ")
            ));
        }
        registry
    }

    /// Register a tool handle, rejecting a tool whose name is already present.
    pub fn register(&mut self, tool: Tool) -> Result<(), RegistryError> {
        let host_system = System::new();
        self.register_for_system(tool, &host_system)
    }

    /// Look up a registered tool by name.
    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|tool| tool.name() == name)
    }

    /// Advertise a preview for every registered tool.
    pub fn preview(&self) -> Vec<ToolPreview> {
        let host_system = System::new();
        self.tools
            .iter()
            .filter_map(|tool| {
                let preview = tool.preview();
                preview.system.supports(&host_system).then_some(preview)
            })
            .collect()
    }
}

// -- Private -- //

impl ToolRegistry {
    fn register_for_system(
        &mut self,
        tool: Tool,
        host_system: &System,
    ) -> Result<(), RegistryError> {
        let preview = tool.preview();
        if !preview.system.supports(host_system) {
            Logger::warning(format!(
                "tool '{}' skipped: system {:?} is incompatible with host {:?}",
                preview.name, preview.system, host_system,
            ));
            return Ok(());
        }

        let name = preview.name;
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
}

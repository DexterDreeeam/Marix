pub mod environment;
pub mod file;
pub mod image;
pub mod network;
pub mod package;
pub mod process;
pub mod shell;
pub mod system;
pub mod text;
pub mod tool_category_list;

pub use environment::ToolCategoryEnvironment;
pub use file::ToolCategoryFile;
pub use image::ToolCategoryImage;
pub use network::ToolCategoryNetwork;
pub use package::ToolCategoryPackage;
pub use process::ToolCategoryProcess;
pub use shell::ToolCategoryShell;
pub use system::ToolCategorySystem;
pub use text::ToolCategoryText;
pub use tool_category_list::TOOL_CATEGORY_LIST;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    File,
    Image,
    Network,
    Shell,
    System,
    Process,
    Environment,
    Package,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryPreview {
    pub name: &'static str,
    pub description: &'static str,
}

use super::CategoryPreview;

pub struct ToolCategoryShell;

impl ToolCategoryShell {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "shell",
        description: "Tools that execute native commands through the operating system shell.",
    };
}

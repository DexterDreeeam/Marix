use super::CategoryPreview;

pub struct ToolCategoryEnvironment;

impl ToolCategoryEnvironment {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "environment",
        description: "Tools that read local environment variables and runtime environment state.",
    };
}

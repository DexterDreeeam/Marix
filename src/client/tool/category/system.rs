use super::CategoryPreview;

pub struct ToolCategorySystem;

impl ToolCategorySystem {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "system",
        description: "Tools that inspect local operating system and machine state.",
    };
}

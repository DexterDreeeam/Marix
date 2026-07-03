use super::CategoryPreview;

pub struct ToolCategoryText;

impl ToolCategoryText {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "text",
        description: "Tools that inspect or transform plain text content.",
    };
}

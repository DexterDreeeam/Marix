use super::CategoryPreview;

pub struct ToolCategoryImage;

impl ToolCategoryImage {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "image",
        description: "Tools that inspect or transform local image files.",
    };
}

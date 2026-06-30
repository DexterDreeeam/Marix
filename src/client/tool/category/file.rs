use super::CategoryPreview;

pub struct ToolCategoryFile;

impl ToolCategoryFile {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "file",
        description: "Tools that read, write, list, or search local file system content.",
    };
}

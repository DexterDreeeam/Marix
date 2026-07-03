use super::CategoryPreview;

pub struct ToolCategoryPackage;

impl ToolCategoryPackage {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "package",
        description: "Tools that query native package manager metadata.",
    };
}

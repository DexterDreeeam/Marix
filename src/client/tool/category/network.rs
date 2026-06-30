use super::CategoryPreview;

pub struct ToolCategoryNetwork;

impl ToolCategoryNetwork {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "network",
        description: "Tools that communicate with network endpoints or resolve network names.",
    };
}

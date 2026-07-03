use super::CategoryPreview;

pub struct ToolCategoryProcess;

impl ToolCategoryProcess {
    pub const PREVIEW: CategoryPreview = CategoryPreview {
        name: "process",
        description: "Tools that inspect or manage local process state.",
    };
}

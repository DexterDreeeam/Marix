use super::{
    CategoryPreview, ToolCategoryEnvironment, ToolCategoryFile, ToolCategoryImage,
    ToolCategoryNetwork, ToolCategoryPackage, ToolCategoryProcess, ToolCategoryShell,
    ToolCategorySystem, ToolCategoryText,
};

pub const TOOL_CATEGORY_LIST: &[CategoryPreview] = &[
    ToolCategoryFile::PREVIEW,
    ToolCategoryImage::PREVIEW,
    ToolCategoryNetwork::PREVIEW,
    ToolCategoryShell::PREVIEW,
    ToolCategorySystem::PREVIEW,
    ToolCategoryProcess::PREVIEW,
    ToolCategoryEnvironment::PREVIEW,
    ToolCategoryPackage::PREVIEW,
    ToolCategoryText::PREVIEW,
];

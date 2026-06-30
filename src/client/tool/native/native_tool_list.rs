use super::{
    DnsLookupTool, EnvironmentTool, HttpRequestTool, ImageInspectTool, ImageTransformTool,
    ListDirectoryTool, PackageQueryTool, ProcessListTool, ReadFileTool, SearchTextTool,
    ShellExecuteTool, SystemInfoTool, WriteFileTool,
};
use crate::client::tool::ToolPreview;

pub const PRIMARY_NATIVE_TOOL_LIST: &[ToolPreview] =
    &[ShellExecuteTool::PREVIEW, HttpRequestTool::PREVIEW];

pub const NATIVE_TOOL_LIST: &[ToolPreview] = &[
    ReadFileTool::PREVIEW,
    WriteFileTool::PREVIEW,
    ListDirectoryTool::PREVIEW,
    SearchTextTool::PREVIEW,
    ImageInspectTool::PREVIEW,
    ImageTransformTool::PREVIEW,
    HttpRequestTool::PREVIEW,
    DnsLookupTool::PREVIEW,
    ShellExecuteTool::PREVIEW,
    SystemInfoTool::PREVIEW,
    ProcessListTool::PREVIEW,
    EnvironmentTool::PREVIEW,
    PackageQueryTool::PREVIEW,
];

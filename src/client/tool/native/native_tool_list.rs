use super::{
    DnsLookupTool, EnvironmentTool, HttpRequestTool, ImageInspectTool, ImageTransformTool,
    ListDirectoryTool, PackageQueryTool, ProcessListTool, ReadFileTool, SearchTextTool,
    ShellExecuteTool, SystemInfoTool, WriteFileTool,
};
use crate::client::tool::{Tool, ToolPreview, ToolType};

pub fn native_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ReadFileTool),
        Box::new(WriteFileTool),
        Box::new(ListDirectoryTool),
        Box::new(SearchTextTool),
        Box::new(ImageInspectTool),
        Box::new(ImageTransformTool),
        Box::new(HttpRequestTool),
        Box::new(DnsLookupTool),
        Box::new(ShellExecuteTool),
        Box::new(SystemInfoTool),
        Box::new(ProcessListTool),
        Box::new(EnvironmentTool),
        Box::new(PackageQueryTool),
    ]
}

pub fn native_tool_list() -> Vec<ToolPreview> {
    native_tools().iter().map(|tool| tool.preview()).collect()
}

pub fn primary_native_tool_list() -> Vec<ToolPreview> {
    native_tools()
        .iter()
        .filter(|tool| tool.tool_type() == ToolType::Primary)
        .map(|tool| tool.preview())
        .collect()
}

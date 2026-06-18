use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignDocument {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub module: DesignModule,
    #[serde(rename = "childModules")]
    pub child_modules: Vec<DesignChildModule>,
    #[serde(default, rename = "exposedGroups")]
    pub exposed_groups: Vec<DesignExposedGroup>,
    pub files: Vec<DesignFile>,
    #[serde(rename = "starMap")]
    pub star_map: DesignStarMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignModule {
    pub path: String,
    pub name: String,
    pub purpose: String,
    #[serde(default, rename = "changeStatus")]
    pub change_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignChildModule {
    pub path: String,
    pub name: String,
    pub purpose: String,
    #[serde(default, rename = "changeStatus")]
    pub change_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignFile {
    pub path: String,
    pub purpose: String,
    #[serde(default, rename = "changeStatus")]
    pub change_status: Option<String>,
    pub items: Vec<DesignItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignItem {
    pub kind: String,
    pub name: String,
    pub category: DesignItemCategory,
    pub signature: String,
    pub details: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default, rename = "lineStart")]
    pub line_start: Option<u32>,
    #[serde(default, rename = "lineEnd")]
    pub line_end: Option<u32>,
    pub implements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignExposedGroup {
    pub name: String,
    pub purpose: String,
    pub elements: Vec<DesignExposedElement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignExposedElement {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub shape: DesignElementShape,
    pub category: DesignItemCategory,
    #[serde(default, rename = "changeStatus")]
    pub change_status: Option<String>,
    #[serde(rename = "sourcePath")]
    pub source_path: String,
    #[serde(default, rename = "lineStart")]
    pub line_start: Option<u32>,
    #[serde(default, rename = "lineEnd")]
    pub line_end: Option<u32>,
    #[serde(default)]
    pub language: Option<String>,
    pub signature: String,
    pub details: String,
    #[serde(default)]
    pub code: Option<String>,
    pub implements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesignItemCategory {
    Interface,
    Implementation,
    Data,
    Module,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesignElementShape {
    Circle,
    Square,
    Triangle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignStarMap {
    pub notes: Vec<String>,
}

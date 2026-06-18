use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ModuleId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarMapInput {
    pub root_path: String,
    pub include_unchanged_modules: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StarMap {
    pub root: ModuleNode,
    pub edges: Vec<ModuleEdge>,
    pub metadata: StarMapMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarMapMetadata {
    pub generated_from_ref: String,
    pub previous_tag: Option<String>,
    pub latest_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleNode {
    pub id: ModuleId,
    pub name: String,
    pub path: String,
    pub kind: ModuleKind,
    pub status: ChangeStatus,
    pub children: Vec<ModuleNode>,
    pub interfaces: Vec<InterfaceSummary>,
    pub data_stores: Vec<DataStoreSummary>,
    pub files: Vec<String>,
    pub layout: NodeLayoutHint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleKind {
    Root,
    Directory,
    RustCrate,
    RustModule,
    PythonPackage,
    WebAssetGroup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeStatus {
    Unchanged,
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceSummary {
    pub name: String,
    pub kind: InterfaceKind,
    pub source_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterfaceKind {
    Trait,
    Struct,
    Enum,
    Function,
    Module,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataStoreSummary {
    pub name: String,
    pub kind: DataStoreKind,
    pub source_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataStoreKind {
    InMemory,
    File,
    Database,
    Config,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleEdge {
    pub from: ModuleId,
    pub to: ModuleId,
    pub kind: ModuleEdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleEdgeKind {
    Contains,
    DependsOn,
    ReadsFrom,
    WritesTo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeLayoutHint {
    pub expanded: bool,
    pub weight: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
}

pub type StarMapResult<T> = Result<T, StarMapError>;

pub trait StarMapProvider {
    fn build_star_map(&self, input: StarMapInput) -> StarMapResult<StarMap>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StarMapError {
    MissingRepository(String),
    InvalidModulePath(String),
}

//! Overview and star-map data model.

mod design;
mod source_map;
mod star_map;

pub use design::{
    DesignChildModule, DesignDocument, DesignElementShape, DesignExposedElement,
    DesignExposedGroup, DesignFile, DesignItem, DesignItemCategory, DesignModule, DesignStarMap,
};
pub use source_map::{
    ChangeLine, ChangeLineKind, ChangeSection, FileChange, RepositorySnapshot, SourceFile,
    SourceLanguage,
};
pub use star_map::{
    ChangeStatus, DataStoreKind, DataStoreSummary, InterfaceKind, InterfaceSummary, ModuleEdge,
    ModuleEdgeKind, ModuleId, ModuleKind, ModuleNode, NodeLayoutHint, StarMap, StarMapInput,
    StarMapMetadata, StarMapProvider, StarMapResult,
};

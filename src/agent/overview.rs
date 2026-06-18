use crate::overview::{
    RepositorySnapshot, StarMap, StarMapInput, StarMapProvider, StarMapResult,
};

use super::runtime::Agent;
use super::types::AgentResult;

pub trait OverviewAgent: Agent + StarMapProvider {
    fn refresh_overview(
        &self,
        request: OverviewRefreshRequest,
    ) -> AgentResult<OverviewRefreshPlan>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewRefreshRequest {
    pub repository: RepositorySnapshot,
    pub star_map: StarMapInput,
    pub options: OverviewOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewOptions {
    pub include_file_view: bool,
    pub include_star_map: bool,
    pub include_diff_sections: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverviewRefreshPlan {
    pub star_map: StarMap,
    pub changed_files: Vec<String>,
    pub overview_paths: Vec<String>,
}

impl OverviewRefreshPlan {
    pub fn from_star_map(star_map: StarMap, changed_files: Vec<String>) -> Self {
        Self {
            star_map,
            changed_files,
            overview_paths: Vec::new(),
        }
    }
}

pub fn build_star_map<T: StarMapProvider>(
    provider: &T,
    input: StarMapInput,
) -> StarMapResult<StarMap> {
    provider.build_star_map(input)
}

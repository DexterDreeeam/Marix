use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentKind {
    Overview,
    Planner,
    Executor,
    Tooling,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentContext {
    pub agent_id: AgentId,
    pub kind: AgentKind,
    pub workspace_root: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInput {
    pub goal: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentOutput {
    pub summary: String,
    pub artifacts: Vec<AgentArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentArtifact {
    pub path: String,
    pub kind: AgentArtifactKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentArtifactKind {
    OverviewManifest,
    StarMap,
    Documentation,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentError {
    InvalidInput(String),
    MissingData(String),
    Backend(String),
}

pub type AgentResult<T> = Result<T, AgentError>;

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::InvalidInput(message) => write!(f, "invalid input: {message}"),
            AgentError::MissingData(message) => write!(f, "missing data: {message}"),
            AgentError::Backend(message) => write!(f, "backend error: {message}"),
        }
    }
}

impl std::error::Error for AgentError {}

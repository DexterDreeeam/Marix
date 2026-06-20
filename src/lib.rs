//! Marix deployment architecture primitives.

pub mod cli;
pub mod config;
pub mod core;

pub use cli::{
    CliUserInterface, UserAttachment, UserCommand, UserInterface, UserMessage, UserOutput,
};
pub use config::{
    CompileMode, DeploymentConfig, LocalModelConfig, ModelEndpoint, PreprocessPlacement,
    RemoteModelConfig, SharedConfig, TransportConfig,
};
pub use core::{
    AgentCore, AgentExecutionAsset, AgentExecutionRequest, AgentExecutionResponse,
    ComputeModelTransport, CoreError, LocalModelBackend, ModelAsset, ModelBackend, ModelError,
    ModelRequest, ModelResponse, PassthroughModelTransport, PassthroughPreprocessor,
    PassthroughUserCoreTransport, PreprocessAsset, PreprocessAssetKind, PreprocessError,
    PreprocessInput, PreprocessOutput, Preprocessor, RemoteModelBackend, UserCoreTransport,
};

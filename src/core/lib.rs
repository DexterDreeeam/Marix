pub mod model;
pub mod preprocess;
pub mod runtime;
pub mod transport;

pub use marix_common::{
    CoreSessionListener, CoreSessionPipe, SessionConfig, SessionPipe, UserInput, UserOutput,
};
pub use marix_config::{config, Config, ConfigError, IConfig};
pub use model::{
    EchoModelBackend, LocalModelBackend, ModelBackend, ModelError, ModelRequest, ModelResponse,
    RemoteModelBackend,
};
pub use preprocess::{PreprocessError, PreprocessOutput, Preprocessor};
pub use runtime::{
    AgentCore, AgentExecutionRequest, AgentExecutionResponse, AgentWorkflow, AgentWorkflowStep,
    CoreError, DEFAULT_AGENT_WORKFLOW,
};
pub use transport::{
    CliCoreTransport, ComputeModelTransport, PassthroughCliCoreTransport, PassthroughModelTransport,
};

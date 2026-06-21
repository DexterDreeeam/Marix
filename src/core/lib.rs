pub mod model;
pub mod preprocess;
pub mod runtime;
pub mod transport;

pub use marix_common::{
    ChatMessageBase, ChatMessageInput, ChatMessageOutput, PipeClient, PipeCloseHandler, PipeError,
    PipeReceiveHandler, PipeResponse, PipeServer, ProtocolConvertError, SessionConfig, UserMessage,
    UserMessageType,
};
pub use marix_config::{config, Config, ConfigError, IConfig};
pub use model::{
    DeepSeekModelBackend, EchoModelBackend, LocalModelBackend, ModelBackend, ModelError,
    ModelRequest, ModelResponse, RemoteModelBackend,
};
pub use preprocess::{PreprocessError, PreprocessOutput, Preprocessor};
pub use runtime::{
    AgentCore, AgentExecutionRequest, AgentExecutionResponse, AgentWorkflow, AgentWorkflowStep,
    CoreError, DEFAULT_AGENT_WORKFLOW,
};
pub use transport::{
    CliCoreTransport, ComputeModelTransport, PassthroughCliCoreTransport, PassthroughModelTransport,
};

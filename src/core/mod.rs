use crate::cli::{UserAttachment, UserCommand, UserOutput};
use crate::config::{LocalModelConfig, RemoteModelConfig, SharedConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessInput {
    pub command: UserCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessOutput {
    pub prompt: String,
    pub tokens: Vec<String>,
    pub assets: Vec<PreprocessAsset>,
}

impl PreprocessOutput {
    pub fn into_execution_request(self) -> AgentExecutionRequest {
        AgentExecutionRequest {
            prompt: self.prompt,
            tokens: self.tokens,
            assets: self
                .assets
                .into_iter()
                .map(AgentExecutionAsset::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessAsset {
    pub name: String,
    pub kind: PreprocessAssetKind,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

impl From<UserAttachment> for PreprocessAsset {
    fn from(attachment: UserAttachment) -> Self {
        Self {
            name: attachment.name,
            kind: PreprocessAssetKind::from_media_type(&attachment.media_type),
            media_type: attachment.media_type,
            bytes: attachment.bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreprocessAssetKind {
    Document,
    Image,
    Other,
}

impl PreprocessAssetKind {
    pub fn from_media_type(media_type: &str) -> Self {
        if media_type.starts_with("image/") {
            Self::Image
        } else if media_type.starts_with("text/") || media_type == "application/pdf" {
            Self::Document
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocessError {
    EmptyInput,
}

impl std::fmt::Display for PreprocessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => write!(formatter, "user input is empty"),
        }
    }
}

impl std::error::Error for PreprocessError {}

pub trait Preprocessor {
    fn preprocess(&self, input: PreprocessInput) -> Result<PreprocessOutput, PreprocessError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PassthroughPreprocessor;

impl Preprocessor for PassthroughPreprocessor {
    fn preprocess(&self, input: PreprocessInput) -> Result<PreprocessOutput, PreprocessError> {
        let prompt = input.command.message.content;
        if prompt.trim().is_empty() && input.command.attachments.is_empty() {
            return Err(PreprocessError::EmptyInput);
        }
        let tokens = prompt.split_whitespace().map(ToOwned::to_owned).collect();
        let assets = input
            .command
            .attachments
            .into_iter()
            .map(PreprocessAsset::from)
            .collect();
        Ok(PreprocessOutput {
            prompt,
            tokens,
            assets,
        })
    }
}

pub trait UserCoreTransport {
    fn forward_user_command(&self, command: UserCommand) -> UserCommand;
    fn forward_user_output(&self, output: UserOutput) -> UserOutput;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PassthroughUserCoreTransport;

impl UserCoreTransport for PassthroughUserCoreTransport {
    fn forward_user_command(&self, command: UserCommand) -> UserCommand {
        command
    }

    fn forward_user_output(&self, output: UserOutput) -> UserOutput {
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentExecutionRequest {
    pub prompt: String,
    pub tokens: Vec<String>,
    pub assets: Vec<AgentExecutionAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentExecutionAsset {
    pub name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

impl From<PreprocessAsset> for AgentExecutionAsset {
    fn from(asset: PreprocessAsset) -> Self {
        Self {
            name: asset.name,
            media_type: asset.media_type,
            bytes: asset.bytes,
        }
    }
}

impl From<AgentExecutionAsset> for ModelAsset {
    fn from(asset: AgentExecutionAsset) -> Self {
        Self {
            name: asset.name,
            media_type: asset.media_type,
            bytes: asset.bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentExecutionResponse {
    pub content: String,
}

pub trait ComputeModelTransport {
    fn forward_to_model(&self, request: ModelRequest) -> ModelRequest;
    fn forward_to_computation(&self, response: ModelResponse) -> ModelResponse;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PassthroughModelTransport;

impl ComputeModelTransport for PassthroughModelTransport {
    fn forward_to_model(&self, request: ModelRequest) -> ModelRequest {
        request
    }

    fn forward_to_computation(&self, response: ModelResponse) -> ModelResponse {
        response
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRequest {
    pub prompt: String,
    pub tokens: Vec<String>,
    pub assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAsset {
    pub name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResponse {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    Unavailable(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(formatter, "model backend unavailable: {reason}"),
        }
    }
}

impl std::error::Error for ModelError {}

pub trait ModelBackend {
    fn generate(&self, request: ModelRequest) -> Result<ModelResponse, ModelError>;
}

pub trait RemoteModelBackend: ModelBackend {
    fn remote_config(&self) -> &RemoteModelConfig;
}

pub trait LocalModelBackend: ModelBackend {
    fn local_config(&self) -> &LocalModelConfig;
}

#[derive(Debug)]
pub enum CoreError {
    Preprocess(PreprocessError),
    Model(ModelError),
}

impl From<PreprocessError> for CoreError {
    fn from(error: PreprocessError) -> Self {
        Self::Preprocess(error)
    }
}

impl From<ModelError> for CoreError {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preprocess(error) => write!(formatter, "preprocess error: {error}"),
            Self::Model(error) => write!(formatter, "model error: {error}"),
        }
    }
}

impl std::error::Error for CoreError {}

#[derive(Debug, Clone)]
pub struct AgentCore {
    config: SharedConfig,
}

impl AgentCore {
    pub fn new(config: SharedConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SharedConfig {
        &self.config
    }

    pub fn run(
        &self,
        command: UserCommand,
        user_transport: &dyn UserCoreTransport,
        preprocessor: &dyn Preprocessor,
        model_transport: &dyn ComputeModelTransport,
        model: &dyn ModelBackend,
    ) -> Result<UserOutput, CoreError> {
        let command = user_transport.forward_user_command(command);
        let preprocessed = preprocessor.preprocess(PreprocessInput { command })?;
        let request = preprocessed.into_execution_request();
        let model_request = model_transport.forward_to_model(ModelRequest {
            prompt: request.prompt,
            tokens: request.tokens,
            assets: request.assets.into_iter().map(ModelAsset::from).collect(),
        });
        let response = model.generate(model_request)?;
        let response = model_transport.forward_to_computation(response);
        Ok(user_transport.forward_user_output(UserOutput {
            content: response.content,
        }))
    }
}

use marix_common::{
    ChatMessageInput, ChatMessageOutput, ProtocolConvertError, UserMessage, UserMessageType,
};
use marix_config::Config;

use super::model::{ModelBackend, ModelError, ModelRequest};
use super::preprocess::{PreprocessError, Preprocessor};
use super::transport::{CliCoreTransport, ComputeModelTransport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentExecutionRequest {
    pub prompt: String,
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentExecutionResponse {
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentWorkflowStep {
    ReceiveInput,
    RunPreprocessor,
    BuildExecutionRequest,
    ForwardRequestToModel,
    GenerateModelResponse,
    ReturnModelResponse,
    ReturnOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentWorkflow {
    steps: &'static [AgentWorkflowStep],
}

impl AgentWorkflow {
    pub const fn new(steps: &'static [AgentWorkflowStep]) -> Self {
        Self { steps }
    }

    pub const fn steps(self) -> &'static [AgentWorkflowStep] {
        self.steps
    }
}

pub const DEFAULT_AGENT_WORKFLOW: AgentWorkflow = AgentWorkflow::new(&[
    AgentWorkflowStep::ReceiveInput,
    AgentWorkflowStep::RunPreprocessor,
    AgentWorkflowStep::BuildExecutionRequest,
    AgentWorkflowStep::ForwardRequestToModel,
    AgentWorkflowStep::GenerateModelResponse,
    AgentWorkflowStep::ReturnModelResponse,
    AgentWorkflowStep::ReturnOutput,
]);

#[derive(Debug)]
pub enum CoreError {
    Preprocess(PreprocessError),
    Model(ModelError),
    Protocol(ProtocolConvertError),
    UnexpectedMessageType(UserMessageType),
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

impl From<ProtocolConvertError> for CoreError {
    fn from(error: ProtocolConvertError) -> Self {
        Self::Protocol(error)
    }
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preprocess(error) => write!(formatter, "preprocess error: {error}"),
            Self::Model(error) => write!(formatter, "model error: {error}"),
            Self::Protocol(error) => write!(formatter, "message protocol error: {error}"),
            Self::UnexpectedMessageType(message_type) => {
                write!(
                    formatter,
                    "unexpected core input message type: {message_type:?}"
                )
            }
        }
    }
}

impl std::error::Error for CoreError {}

#[derive(Debug, Clone)]
pub struct AgentCore {
    config: Config,
    workflow: AgentWorkflow,
}

impl AgentCore {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            workflow: DEFAULT_AGENT_WORKFLOW,
        }
    }

    pub fn with_workflow(config: Config, workflow: AgentWorkflow) -> Self {
        Self { config, workflow }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn workflow(&self) -> AgentWorkflow {
        self.workflow
    }

    pub fn run(
        &self,
        input: ChatMessageInput,
        cli_transport: &dyn CliCoreTransport,
        preprocessor: &Preprocessor,
        model_transport: &dyn ComputeModelTransport,
        model: &dyn ModelBackend,
    ) -> Result<ChatMessageOutput, CoreError> {
        let input = cli_transport.forward_input(input);
        let preprocessed = preprocessor.run(input)?;
        let request = AgentExecutionRequest {
            prompt: preprocessed.prompt,
            tokens: preprocessed.tokens,
        };
        let model_request = model_transport.forward_to_model(ModelRequest {
            prompt: request.prompt,
            tokens: request.tokens,
        });
        let response = model.generate(model_request)?;
        let response = model_transport.forward_to_computation(response);
        Ok(cli_transport.forward_output(ChatMessageOutput::new(response.content)))
    }

    pub fn run_message_bytes(
        &self,
        input_bytes: &[u8],
        cli_transport: &dyn CliCoreTransport,
        preprocessor: &Preprocessor,
        model_transport: &dyn ComputeModelTransport,
        model: &dyn ModelBackend,
    ) -> Result<Vec<u8>, CoreError> {
        let message_type = UserMessageType::classify(input_bytes)?;
        if message_type != UserMessageType::ChatMessageInput {
            return Err(CoreError::UnexpectedMessageType(message_type));
        }

        let input = <ChatMessageInput as UserMessage>::from_bytes(input_bytes)?;
        let output = self.run(input, cli_transport, preprocessor, model_transport, model)?;
        <ChatMessageOutput as UserMessage>::to_bytes(&output).map_err(CoreError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EchoModelBackend;
    use crate::transport::{PassthroughCliCoreTransport, PassthroughModelTransport};

    #[test]
    fn runs_message_protocol_bytes() {
        let core = AgentCore::new(Config::empty());
        let input_bytes = ChatMessageInput::new("hello core").to_bytes().unwrap();

        let output_bytes = core
            .run_message_bytes(
                &input_bytes,
                &PassthroughCliCoreTransport,
                &Preprocessor,
                &PassthroughModelTransport,
                &EchoModelBackend,
            )
            .unwrap();

        let output = <ChatMessageOutput as UserMessage>::from_bytes(&output_bytes).unwrap();
        assert_eq!(output.content(), "hello core");
    }

    #[test]
    fn rejects_output_message_as_core_input() {
        let core = AgentCore::new(Config::empty());
        let input_bytes = ChatMessageOutput::new("wrong direction")
            .to_bytes()
            .unwrap();

        let error = core
            .run_message_bytes(
                &input_bytes,
                &PassthroughCliCoreTransport,
                &Preprocessor,
                &PassthroughModelTransport,
                &EchoModelBackend,
            )
            .unwrap_err();

        match error {
            CoreError::UnexpectedMessageType(UserMessageType::ChatMessageOutput) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}

use marix_common::{ChatMessageInput, ChatMessageOutput};
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
}

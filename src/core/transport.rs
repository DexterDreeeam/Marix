use marix_common::{ChatMessageInput, ChatMessageOutput};

use super::model::{ModelRequest, ModelResponse};

pub trait CliCoreTransport {
    fn forward_input(&self, input: ChatMessageInput) -> ChatMessageInput;
    fn forward_output(&self, output: ChatMessageOutput) -> ChatMessageOutput;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PassthroughCliCoreTransport;

impl CliCoreTransport for PassthroughCliCoreTransport {
    fn forward_input(&self, input: ChatMessageInput) -> ChatMessageInput {
        input
    }

    fn forward_output(&self, output: ChatMessageOutput) -> ChatMessageOutput {
        output
    }
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

use marix_common::{
    ChatMessageInput, ChatMessageOutput, PipeError, PipeResponse, PipeServer, UserMessage,
    UserMessageType,
};

use crate::model_backend::ModelBackend;
use crate::model_backend_deepseek::ModelBackendDeepseek;
use crate::preprocess::Preprocessor;

#[derive(Debug)]
pub struct CorePipeServer {
    preprocessor: Preprocessor,
    model_backend: Box<dyn ModelBackend>,
    closed: bool,
}

impl CorePipeServer {
    pub fn new(model_backend: Box<dyn ModelBackend>) -> Self {
        Self {
            preprocessor: Preprocessor,
            model_backend,
            closed: false,
        }
    }
}

impl Default for CorePipeServer {
    fn default() -> Self {
        Self::new(Box::new(ModelBackendDeepseek::from_env()))
    }
}

impl PipeServer for CorePipeServer {
    fn send(&mut self, message: impl UserMessage) -> Result<PipeResponse, PipeError> {
        if self.closed {
            return Err(PipeError::SendFailed(
                "core pipe server is closed".to_owned(),
            ));
        }

        if message.get_type() != UserMessageType::ChatMessageOutput {
            return Ok(PipeResponse::rejected(
                "core pipe output must be ChatMessageOutput",
            ));
        }

        let bytes = message
            .to_bytes()
            .map_err(|error| PipeError::SendFailed(error.to_string()))?;
        let output = <ChatMessageOutput as UserMessage>::from_bytes(&bytes)
            .map_err(|error| PipeError::SendFailed(error.to_string()))?;
        if output.is_empty() {
            return Ok(PipeResponse::rejected("core output message is empty"));
        }

        println!("{}", output.content());
        Ok(PipeResponse::accepted())
    }

    fn on_receive(&mut self, message: impl UserMessage) -> Result<PipeResponse, PipeError> {
        if self.closed {
            return Err(PipeError::ReceiveFailed(
                "core pipe server is closed".to_owned(),
            ));
        }

        if message.get_type() != UserMessageType::ChatMessageInput {
            return Ok(PipeResponse::rejected(
                "core pipe input must be ChatMessageInput",
            ));
        }

        let bytes = message
            .to_bytes()
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        let input = <ChatMessageInput as UserMessage>::from_bytes(&bytes)
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        let preprocessed = match self.preprocessor.run(input) {
            Ok(preprocessed) => preprocessed,
            Err(error) => return Ok(PipeResponse::rejected(error.to_string())),
        };
        let output = self
            .model_backend
            .wait_response(preprocessed)
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;

        self.send(ChatMessageOutput::new(output.content))
    }

    fn close(&mut self) -> Result<PipeResponse, PipeError> {
        self.on_close()
    }

    fn on_close(&mut self) -> Result<PipeResponse, PipeError> {
        self.closed = true;
        Ok(PipeResponse::accepted())
    }
}

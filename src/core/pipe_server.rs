use std::io::Write;

use marix_common::{
    ChatMessageInput, ChatMessageOutput, CompleteMessage, DynamicResponseSignal, Pipe, PipeError,
    PipeResponse, UserMessage, UserMessageType,
};

use crate::model_backend::{ModelBackend, ModelBackendDeepseek};
use crate::preprocess::Preprocessor;

#[derive(Debug)]
pub struct PipeServer {
    preprocessor: Preprocessor,
    model_backend: Box<dyn ModelBackend>,
    closed: bool,
}

impl PipeServer {
    pub fn new(model_backend: Box<dyn ModelBackend>) -> Self {
        Self {
            preprocessor: Preprocessor,
            model_backend,
            closed: false,
        }
    }
}

impl Default for PipeServer {
    fn default() -> Self {
        Self::new(Box::new(ModelBackendDeepseek::new()))
    }
}

impl Pipe for PipeServer {
    fn send(&mut self, message: impl UserMessage) -> Result<PipeResponse, PipeError> {
        if self.closed {
            return Err(PipeError::SendFailed(
                "core pipe server is closed".to_owned(),
            ));
        }

        match message.get_type() {
            UserMessageType::ChatMessageOutput => {
                let bytes = message
                    .to_bytes()
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                let output = <ChatMessageOutput as UserMessage>::from_bytes(&bytes)
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                if output.is_empty() {
                    return Ok(PipeResponse::rejected("core output message is empty"));
                }

                print!("{}", output.content());
                std::io::stdout()
                    .flush()
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                Ok(PipeResponse::accepted())
            }
            UserMessageType::CompleteMessage => {
                let bytes = message
                    .to_bytes()
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                let complete = <CompleteMessage as UserMessage>::from_bytes(&bytes)
                    .map_err(|error| PipeError::SendFailed(error.to_string()))?;
                if complete.is_empty() {
                    return Ok(PipeResponse::rejected(
                        "complete message correlation id is empty",
                    ));
                }

                println!();
                Ok(PipeResponse::accepted())
            }
            UserMessageType::ChatMessageInput => Ok(PipeResponse::rejected(
                "core pipe output must be ChatMessageOutput or CompleteMessage",
            )),
        }
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

        let correlation_id = message.correlation_id().to_owned();
        let bytes = message
            .to_bytes()
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        let input = <ChatMessageInput as UserMessage>::from_bytes(&bytes)
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        let preprocessed = match self.preprocessor.run(input) {
            Ok(preprocessed) => preprocessed,
            Err(error) => return Ok(PipeResponse::rejected(error.to_string())),
        };
        let response = self
            .model_backend
            .request_response(preprocessed)
            .map_err(|error| PipeError::ReceiveFailed(error.to_string()))?;
        let mut sent_length = 0;
        loop {
            match response.wait(None) {
                DynamicResponseSignal::Changed => {
                    let output = response.get();
                    if output.content.len() > sent_length {
                        let content = output.content[sent_length..].to_owned();
                        sent_length = output.content.len();
                        self.send(ChatMessageOutput::new(correlation_id.clone(), content))?;
                    }
                }
                DynamicResponseSignal::Finished => {
                    let output = response.get();
                    if output.content.len() > sent_length {
                        let content = output.content[sent_length..].to_owned();
                        self.send(ChatMessageOutput::new(correlation_id.clone(), content))?;
                    }
                    return self.send(CompleteMessage::new(correlation_id));
                }
                DynamicResponseSignal::Failed(reason) => {
                    return Err(PipeError::ReceiveFailed(reason));
                }
                DynamicResponseSignal::TimedOut => {}
            }
        }
    }

    fn close(&mut self) -> Result<PipeResponse, PipeError> {
        self.on_close()
    }

    fn on_close(&mut self) -> Result<PipeResponse, PipeError> {
        self.closed = true;
        Ok(PipeResponse::accepted())
    }
}

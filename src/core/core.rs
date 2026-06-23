use marix_common::{
    ChatMessageInput, ChatMessageOutput, PipeError, PipeResponse, PipeServer, UserMessage,
    UserMessageType,
};

use crate::preprocess::Preprocessor;

#[derive(Debug, Clone, Default)]
pub struct CorePipeServer {
    preprocessor: Preprocessor,
    closed: bool,
}

impl CorePipeServer {
    fn decode_message<Message: UserMessage>(
        message: impl UserMessage,
        pipe_error: fn(String) -> PipeError,
    ) -> Result<Message, PipeError> {
        let bytes = message
            .to_bytes()
            .map_err(|error| pipe_error(error.to_string()))?;
        Message::from_bytes(&bytes).map_err(|error| pipe_error(error.to_string()))
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

        let output = Self::decode_message::<ChatMessageOutput>(message, PipeError::SendFailed)?;
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

        let input = Self::decode_message::<ChatMessageInput>(message, PipeError::ReceiveFailed)?;
        let preprocessed = match self.preprocessor.run(input) {
            Ok(preprocessed) => preprocessed,
            Err(error) => return Ok(PipeResponse::rejected(error.to_string())),
        };

        self.send(ChatMessageOutput::new(preprocessed.prompt))
    }

    fn close(&mut self) -> Result<PipeResponse, PipeError> {
        self.on_close()
    }

    fn on_close(&mut self) -> Result<PipeResponse, PipeError> {
        self.closed = true;
        Ok(PipeResponse::accepted())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receives_chat_message_through_preprocessor() {
        let mut pipe = CorePipeServer::default();

        let response = pipe.on_receive(ChatMessageInput::new("hello core")).unwrap();

        assert_eq!(response, PipeResponse::Accepted);
    }

    #[test]
    fn rejects_empty_chat_message() {
        let mut pipe = CorePipeServer::default();

        let response = pipe.on_receive(ChatMessageInput::new("   ")).unwrap();

        assert_eq!(response, PipeResponse::Rejected("user input is empty".to_owned()));
    }

    #[test]
    fn on_receive_rejects_wrong_message_type() {
        let mut pipe = CorePipeServer::default();

        let response = pipe
            .on_receive(ChatMessageOutput::new("wrong direction"))
            .unwrap();

        assert_eq!(
            response,
            PipeResponse::Rejected("core pipe input must be ChatMessageInput".to_owned())
        );
    }

    #[test]
    fn send_rejects_wrong_message_type() {
        let mut pipe = CorePipeServer::default();

        let response = pipe.send(ChatMessageInput::new("wrong direction")).unwrap();

        assert_eq!(
            response,
            PipeResponse::Rejected("core pipe output must be ChatMessageOutput".to_owned())
        );
    }

    #[test]
    fn close_blocks_later_receive() {
        let mut pipe = CorePipeServer::default();

        pipe.close().unwrap();
        let error = pipe.on_receive(ChatMessageInput::new("after close")).unwrap_err();

        assert!(matches!(error, PipeError::ReceiveFailed(_)));
    }
}

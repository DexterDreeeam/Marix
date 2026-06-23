use super::{PipeError, PipeResponse};
use crate::message::UserMessage;

pub trait Pipe {
    fn send(&mut self, message: impl UserMessage) -> Result<PipeResponse, PipeError>;

    fn on_receive(&mut self, message: impl UserMessage) -> Result<PipeResponse, PipeError>;

    fn close(&mut self) -> Result<PipeResponse, PipeError>;

    fn on_close(&mut self) -> Result<PipeResponse, PipeError>;
}

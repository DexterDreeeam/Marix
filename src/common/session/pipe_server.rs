use super::{PipeCloseHandler, PipeError, PipeReceiveHandler, PipeResponse};

pub trait PipeServer<Outgoing, Incoming> {
    fn send(&mut self, message: Outgoing) -> Result<PipeResponse, PipeError>;

    fn on_receive(
        &mut self,
        handler: PipeReceiveHandler<Incoming>,
    ) -> Result<PipeResponse, PipeError>;

    fn close(&mut self) -> Result<PipeResponse, PipeError>;

    fn on_close(&mut self, handler: PipeCloseHandler) -> Result<PipeResponse, PipeError>;
}

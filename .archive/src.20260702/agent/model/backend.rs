use std::fmt;
use std::sync::mpsc::Receiver;

use crate::agent::engine::ModelTaskStepKind;

use super::error::ModelBackendError;

pub trait ModelBackend: fmt::Debug + Send {
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<Receiver<ModelResponse>, ModelBackendError>;
}

pub(super) trait ModelBackendImpl: fmt::Debug + Send {
    fn ready(&self) -> Result<(), ModelBackendError>;

    fn send(&mut self, request: ModelRequest)
        -> Result<Receiver<ModelResponse>, ModelBackendError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRequest {
    pub step: ModelTaskStepKind,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResponse {
    Content(String),
    Failed(ModelBackendError),
}

// -- Private -- //

impl<T> ModelBackend for T
where
    T: ModelBackendImpl,
{
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<Receiver<ModelResponse>, ModelBackendError> {
        self.ready()?;
        self.send(request)
    }
}

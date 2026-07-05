use std::fmt;

use crate::Step;
use marix_common::Receiver;

use super::error::ModelBackendError;

pub trait ModelBackend: fmt::Debug + Send {
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<Receiver<ModelResponse>, ModelBackendError>;
}

pub(super) trait ModelBackendImpl: fmt::Debug + Send {
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<Receiver<ModelResponse>, ModelBackendError>;
}

#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub step: Step,
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
        <T as ModelBackendImpl>::request(self, request)
    }
}

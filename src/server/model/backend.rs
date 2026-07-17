use std::fmt;

use marix_common::{AsyncReceiver, Receiver};
use marix_protocol::{ContextChain, RelaySignature, ToolPreview};

use super::error::ModelBackendError;

pub type ModelResponseReceiver = Receiver<ModelResponse>;
pub type ModelResponseAsyncReceiver = AsyncReceiver<ModelResponse>;

pub trait ModelBackend: fmt::Debug + Send {
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseReceiver, ModelBackendError>;

    fn request_async(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseAsyncReceiver, ModelBackendError>;
}

pub(super) trait ModelBackendImpl: fmt::Debug + Send {
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseReceiver, ModelBackendError>;

    fn request_async(
        &mut self,
        _request: ModelRequest,
    ) -> Result<ModelResponseAsyncReceiver, ModelBackendError> {
        panic!("not implemented")
    }
}

#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub relay: RelaySignature,
    pub system: String,
    pub context: ContextChain,
    pub prompt: String,
    pub tools: Vec<ToolPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResponse {
    pub content: String,
    pub seq: usize,
    pub complete: bool,
}

// -- Private -- //

impl<T> ModelBackend for T
where
    T: ModelBackendImpl,
{
    fn request(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseReceiver, ModelBackendError> {
        <T as ModelBackendImpl>::request(self, request)
    }

    fn request_async(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseAsyncReceiver, ModelBackendError> {
        <T as ModelBackendImpl>::request_async(self, request)
    }
}

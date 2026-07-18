use std::fmt;

use marix_common::AsyncReceiver;
use marix_protocol::{RelaySignature, ToolPreview};

use super::error::ModelBackendError;

pub type ModelResponseStream = AsyncReceiver<ModelResponse>;

pub trait ModelBackend: fmt::Debug + Send {
    fn request_stream(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseStream, ModelBackendError>;
}

pub(super) trait ModelBackendImpl: fmt::Debug + Send {
    fn request_stream(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseStream, ModelBackendError>;
}

#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub relay: RelaySignature,
    pub system: String,
    pub prompts: Vec<String>,
    pub tools: Option<Vec<ToolPreview>>,
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
    fn request_stream(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseStream, ModelBackendError> {
        <T as ModelBackendImpl>::request_stream(self, request)
    }
}

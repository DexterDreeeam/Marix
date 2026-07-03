use std::fmt;

use super::backend::ModelBackendImpl;
use super::{ModelBackendError, ModelRequest, ModelResponse};
use marix_common::Receiver;

#[derive(Clone)]
pub struct DeepseekBackend;

impl DeepseekBackend {
    pub fn new() -> Self {
        panic!("not implemented")
    }
}

// -- Private -- //

impl ModelBackendImpl for DeepseekBackend {
    fn ready(&self) -> Result<(), ModelBackendError> {
        panic!("not implemented")
    }

    fn send(
        &mut self,
        request: ModelRequest,
    ) -> Result<Receiver<ModelResponse>, ModelBackendError> {
        panic!("not implemented")
    }
}

impl fmt::Debug for DeepseekBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("not implemented")
    }
}

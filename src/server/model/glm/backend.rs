use std::fmt;

use marix_common::GlmConfig;

use crate::model::backend::ModelBackendImpl;
use crate::model::openai::OpenAiCore;
use crate::model::{ModelBackendError, ModelRequest, ModelResponseStream};

#[derive(Clone)]
pub struct GlmBackend {
    core: OpenAiCore,
}

impl GlmBackend {
    pub fn new(config: GlmConfig) -> Self {
        Self {
            core: OpenAiCore::new("Glm", config.endpoint, config.model, config.api_key),
        }
    }
}

// -- Private -- //

impl ModelBackendImpl for GlmBackend {
    fn request_stream(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseStream, ModelBackendError> {
        self.core.request_stream(request)
    }
}

impl fmt::Debug for GlmBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.core, formatter)
    }
}

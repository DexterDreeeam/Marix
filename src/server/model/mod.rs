pub mod deepseek;
pub mod error;
pub mod glm;

pub use backend::{ModelBackend, ModelRequest, ModelResponse, ModelResponseStream};
pub use deepseek::DeepseekBackend;
pub use error::ModelBackendError;
pub use glm::GlmBackend;

// -- Private -- //

mod backend;
mod openai;

pub mod backend_deepseek;
pub mod error;

mod backend;

pub use backend::{ModelBackend, ModelBackendType, ModelRequest, ModelResponse};
pub use backend_deepseek::DeepseekBackend;
pub use error::ModelBackendError;

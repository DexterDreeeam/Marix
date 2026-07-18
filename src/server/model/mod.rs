pub mod deepseek;
pub mod error;

pub use backend::{ModelBackend, ModelRequest, ModelResponse, ModelResponseStream};
pub use deepseek::DeepseekBackend;
pub use error::ModelBackendError;

// -- Private -- //

mod backend;

pub mod backend_deepseek;
pub mod error;

pub use backend::{ModelBackend, ModelRequest, ModelResponse};
pub use backend_deepseek::DeepseekBackend;
pub use error::ModelBackendError;
pub(crate) use model_context::ModelContext;

// -- Private -- //

mod backend;
mod model_context;

pub mod backend_deepseek;
pub mod error;

pub use backend::{
    ModelBackend, ModelRequest, ModelResponse, ModelResponseAsyncReceiver, ModelResponseReceiver,
};
pub use backend_deepseek::DeepseekBackend;
pub use error::ModelBackendError;

// -- Private -- //

mod backend;

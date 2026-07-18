pub mod error;
pub mod prompt;

pub use error::PromptError;
pub use prompt::Prompt;

pub(crate) use system_map::{MessagePrompt, SystemPrompt};

// -- Private -- //

mod system_map;

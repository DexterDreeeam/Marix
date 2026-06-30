pub mod entry;
pub mod error;
pub mod logger;

pub use entry::{LogMessage, LogTag};
pub use error::LoggingError;
pub use logger::{debug, log};

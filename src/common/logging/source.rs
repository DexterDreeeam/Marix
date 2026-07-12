use crate::external::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogSource {
    Host,
    Client,
    Server,
}

impl Default for LogSource {
    fn default() -> Self {
        Self::Server
    }
}

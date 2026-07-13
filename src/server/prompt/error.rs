use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptError {
    MissingParameters(Vec<String>),
}

impl fmt::Display for PromptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParameters(parameters) => write!(
                formatter,
                "prompt parameters are not injected: {}",
                parameters.join(", ")
            ),
        }
    }
}

impl std::error::Error for PromptError {}

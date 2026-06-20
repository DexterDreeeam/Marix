#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocessError {
    EmptyInput,
}

impl std::fmt::Display for PreprocessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => write!(formatter, "user input is empty"),
        }
    }
}

impl std::error::Error for PreprocessError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessOutput {
    pub prompt: String,
    pub tokens: Vec<String>,
}

use marix_common::ChatMessageInput;

use super::{PreprocessError, PreprocessOutput};

#[derive(Debug, Clone, Copy, Default)]
pub struct Preprocessor;

impl Preprocessor {
    pub fn run(&self, input: ChatMessageInput) -> Result<PreprocessOutput, PreprocessError> {
        let prompt = input.chat_text().to_owned();
        if prompt.trim().is_empty() {
            return Err(PreprocessError::EmptyInput);
        }
        let tokens = prompt.split_whitespace().map(ToOwned::to_owned).collect();
        Ok(PreprocessOutput { prompt, tokens })
    }
}

use super::{ChatMessageInput, ChatMessageOutput};

pub trait Interface {
    fn read_input(&mut self) -> Option<ChatMessageInput>;
    fn render_output(&mut self, output: ChatMessageOutput);
}

#[derive(Debug, Default)]
pub struct CliInterface {
    outputs: Vec<ChatMessageOutput>,
}

impl CliInterface {
    pub fn outputs(&self) -> &[ChatMessageOutput] {
        &self.outputs
    }
}

impl Interface for CliInterface {
    fn read_input(&mut self) -> Option<ChatMessageInput> {
        None
    }

    fn render_output(&mut self, output: ChatMessageOutput) {
        self.outputs.push(output);
    }
}

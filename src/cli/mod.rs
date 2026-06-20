use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMessage {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserAttachment {
    pub name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCommand {
    pub message: UserMessage,
    pub attachments: Vec<UserAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOutput {
    pub content: String,
}

pub trait UserInterface {
    fn read_command(&mut self) -> Option<UserCommand>;
    fn render_output(&mut self, output: UserOutput);
}

#[derive(Debug, Default)]
pub struct CliUserInterface {
    outputs: Vec<UserOutput>,
}

impl CliUserInterface {
    pub fn outputs(&self) -> &[UserOutput] {
        &self.outputs
    }
}

impl UserInterface for CliUserInterface {
    fn read_command(&mut self) -> Option<UserCommand> {
        None
    }

    fn render_output(&mut self, output: UserOutput) {
        self.outputs.push(output);
    }
}

use super::{UserInput, UserOutput};

pub trait Interface {
    fn read_input(&mut self) -> Option<UserInput>;
    fn render_output(&mut self, output: UserOutput);
}

#[derive(Debug, Default)]
pub struct CliInterface {
    outputs: Vec<UserOutput>,
}

impl CliInterface {
    pub fn outputs(&self) -> &[UserOutput] {
        &self.outputs
    }
}

impl Interface for CliInterface {
    fn read_input(&mut self) -> Option<UserInput> {
        None
    }

    fn render_output(&mut self, output: UserOutput) {
        self.outputs.push(output);
    }
}

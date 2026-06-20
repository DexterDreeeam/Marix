use super::{Output, UserInput};

pub trait Interface {
    fn read_input(&mut self) -> Option<UserInput>;
    fn render_output(&mut self, output: Output);
}

#[derive(Debug, Default)]
pub struct CliInterface {
    outputs: Vec<Output>,
}

impl CliInterface {
    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }
}

impl Interface for CliInterface {
    fn read_input(&mut self) -> Option<UserInput> {
        None
    }

    fn render_output(&mut self, output: Output) {
        self.outputs.push(output);
    }
}

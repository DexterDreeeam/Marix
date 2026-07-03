use std::path::{Path, PathBuf};

use marix_common::ToolPreview;

pub struct Tool {
    path: PathBuf,
}

impl Tool {
    pub fn load(path: &Path) -> Self {
        panic!("not implemented")
    }

    pub fn name(&self) -> String {
        panic!("not implemented")
    }

    pub fn description(&self) -> String {
        panic!("not implemented")
    }

    pub fn input_schema(&self) -> String {
        panic!("not implemented")
    }

    pub fn output_schema(&self) -> String {
        panic!("not implemented")
    }

    pub fn preview(&self) -> ToolPreview {
        panic!("not implemented")
    }

    pub fn execute(&self, input: &str) -> String {
        panic!("not implemented")
    }
}

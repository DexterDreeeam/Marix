use marix_common::{System, WorkQueue};
use marix_protocol::{TaskPreview, TaskSignature, ToolPreview};

use crate::task::Task;

pub struct SessionContext {
    pub system: Option<System>,
    pub tasks: WorkQueue<TaskSignature, Task>,
    pub tools: Vec<ToolPreview>,
}

impl SessionContext {
    pub fn snapshot(&self) -> SessionContextSnapshot {
        SessionContextSnapshot {
            system: self.system,
            tasks: self
                .tasks
                .list()
                .into_iter()
                .map(|task| task.preview())
                .collect(),
            tools: self.tools.clone(),
        }
    }

    pub fn is_valid_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name == name)
    }
}

pub struct SessionContextSnapshot {
    pub system: Option<System>,
    pub tasks: Vec<TaskPreview>,
    pub tools: Vec<ToolPreview>,
}

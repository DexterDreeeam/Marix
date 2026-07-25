use marix_common::{LogLevel, Logger};

use crate::{TaskId, TaskSignature};

pub trait TaskLogging {
    fn logger(&self) -> TaskLogger;

    fn info(&self, message: impl Into<String>) {
        self.logger().info(message);
    }

    fn warning(&self, message: impl Into<String>) {
        self.logger().warning(message);
    }

    fn error(&self, message: impl Into<String>) {
        self.logger().error(message);
    }

    fn debug(&self, message: impl Into<String>) {
        self.logger().debug(message);
    }

    fn info_tagged(
        &self,
        message: impl Into<String>,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.logger().info_tagged(message, tags);
    }
}

#[derive(Debug, Clone)]
pub struct TaskLogger {
    task_id: TaskId,
}

impl From<TaskId> for TaskLogger {
    fn from(task_id: TaskId) -> Self {
        Self { task_id }
    }
}

impl From<TaskSignature> for TaskLogger {
    fn from(signature: TaskSignature) -> Self {
        Self::from(signature.id)
    }
}

impl TaskLogger {
    pub fn info(&self, message: impl Into<String>) {
        self.emit(LogLevel::Info, message, Vec::<String>::new());
    }

    pub fn debug(&self, message: impl Into<String>) {
        self.emit(LogLevel::Debug, message, Vec::<String>::new());
    }

    pub fn warning(&self, message: impl Into<String>) {
        self.emit(LogLevel::Warning, message, Vec::<String>::new());
    }

    pub fn error(&self, message: impl Into<String>) {
        self.emit(LogLevel::Error, message, Vec::<String>::new());
    }

    pub fn info_tagged(
        &self,
        message: impl Into<String>,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.emit(LogLevel::Info, message, tags);
    }

    pub fn warning_tagged(
        &self,
        message: impl Into<String>,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.emit(LogLevel::Warning, message, tags);
    }

    pub fn error_tagged(
        &self,
        message: impl Into<String>,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.emit(LogLevel::Error, message, tags);
    }
}

// -- Private -- //

impl TaskLogger {
    fn emit(
        &self,
        level: LogLevel,
        message: impl Into<String>,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) {
        Logger::emit_for_task(self.task_id.0, level, message, tags);
    }
}

#[cfg(test)]
mod tests {
    use super::TaskLogger;
    use crate::{TaskId, TaskSignature};

    #[test]
    fn task_id_and_signature_convert_to_their_task_logger() {
        let task_id = TaskId::new();
        let from_id = TaskLogger::from(task_id.clone());
        assert_eq!(from_id.task_id, task_id);

        let signature = TaskSignature::new("test".to_owned());
        let expected = signature.id.clone();
        let from_signature = TaskLogger::from(signature);
        assert_eq!(from_signature.task_id, expected);
    }
}

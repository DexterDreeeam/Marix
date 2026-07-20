use marix_common::external::*;
use marix_common::{Logger, build_async_channel};

use super::DeepseekBackend;
use crate::model::backend::ModelBackendImpl;
use crate::model::{ModelBackendError, ModelRequest, ModelResponseStream};

// -- Private -- //

impl ModelBackendImpl for DeepseekBackend {
    fn request_stream(
        &mut self,
        request: ModelRequest,
    ) -> Result<ModelResponseStream, ModelBackendError> {
        Logger::debug(format!(
            "deepseek stream request: model '{}'",
            self.config.model.trim()
        ));
        let task_id = request.relay.intent.task.id.0.to_string();
        let native_tools = request.tools.is_some();
        let raw = match self.build_payload(&request) {
            Ok(raw) => raw,
            Err(error) => {
                Logger::error(format!("[Model Relay][{task_id}][Request] {error}"));
                return Err(error);
            }
        };
        Logger::log(format!("[Model Relay][{task_id}][Request] {raw}"));
        let config = self.config.clone();
        let client = self.async_client.clone();
        let (sender, receiver) = build_async_channel();
        tokio::spawn(async move {
            if let Err(error) =
                Self::request_stream_response(client, config, raw, &task_id, native_tools, sender)
                    .await
            {
                Logger::error(format!("[Model Relay][{task_id}][Response] {error}"));
            }
        });

        Ok(receiver)
    }
}

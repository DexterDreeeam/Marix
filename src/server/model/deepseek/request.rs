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
        let native_tools = request.tools.is_some();
        let raw = self.build_payload(&request)?;
        Logger::log(format!("[Model Relay][Request] {raw}"));
        let config = self.config.clone();
        let client = self.async_client.clone();
        let (sender, receiver) = build_async_channel();
        tokio::spawn(async move {
            if let Err(error) =
                Self::request_stream_response(client, config, raw, native_tools, sender).await
            {
                Logger::error(format!("deepseek stream response failed: {error}"));
            }
        });

        Ok(receiver)
    }
}

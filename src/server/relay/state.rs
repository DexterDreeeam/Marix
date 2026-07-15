use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use marix_common::{
    AsyncReceiver, AsyncSender, Config, ModelBackend as ConfigModelBackend, build_async_channel,
};
use marix_protocol::{RelayEvent, RelayRequest, RelaySignature, RelayStatus};

use crate::model::{DeepseekBackend, ModelBackend};
use crate::task::TaskAccess;

pub struct RelayState {
    pub access: Arc<TaskAccess>,
    pub signature: RelaySignature,
    pub prompt: String,
    pub status: StdMutex<RelayStatus>,
    pub output: StdMutex<BTreeMap<usize, String>>,
    pub final_signal: StdMutex<Option<usize>>,
    pub model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub relay_tx: AsyncSender<RelayEvent>,
    pub relay_rx: StdMutex<Option<AsyncReceiver<RelayEvent>>>,
}

// -- Private -- //

impl RelayState {
    pub(crate) fn new(
        access: Arc<TaskAccess>,
        request: RelayRequest,
    ) -> Result<Self, String> {
        let (relay_tx, relay_rx) = build_async_channel();
        let config = Config::load().map_err(|error| format!("failed to load config: {error}"))?;
        let model_backend: Box<dyn ModelBackend> = match config.model.backend {
            ConfigModelBackend::Deepseek => {
                let backend =
                    std::panic::catch_unwind(DeepseekBackend::new).map_err(|payload| {
                        let detail = if let Some(message) = payload.downcast_ref::<String>() {
                            message.clone()
                        } else if let Some(message) = payload.downcast_ref::<&str>() {
                            (*message).to_owned()
                        } else {
                            "unknown backend construction panic".to_owned()
                        };
                        format!("failed to construct model backend: {detail}")
                    })?;
                Box::new(backend)
            }
        };
        Ok(Self {
            access,
            signature: request.signature,
            prompt: request.prompt,
            status: StdMutex::new(RelayStatus::Created),
            output: StdMutex::new(BTreeMap::new()),
            final_signal: StdMutex::new(None),
            model_backend: StdMutex::new(model_backend),
            relay_tx,
            relay_rx: StdMutex::new(Some(relay_rx)),
        })
    }
}

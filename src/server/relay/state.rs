use std::collections::BTreeMap;
use std::sync::Mutex as StdMutex;

use marix_common::external::*;
use marix_common::{
    AsyncReceiver, AsyncSender, Config, ModelBackend as ConfigModelBackend, build_async_channel,
};
use marix_protocol::{RelayEvent, RelayRequest, RelaySignature};

use crate::model::{DeepseekBackend, ModelBackend};
use crate::task::TaskAccess;

pub(super) struct RelayState {
    pub(super) access: TaskAccess,
    pub(super) signature: RelaySignature,
    pub(super) relay_tx: AsyncSender<RelayEvent>,
    pub(super) relay_rx: StdMutex<Option<AsyncReceiver<RelayEvent>>>,
    pub(super) model_backend: StdMutex<Box<dyn ModelBackend>>,
    pub(super) prompt: String,
    pub(super) output: StdMutex<BTreeMap<usize, String>>,
    pub(super) final_signal: StdMutex<Option<usize>>,
}

impl RelayState {
    pub fn output(&self) -> String {
        self.output
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .cloned()
            .collect()
    }

    pub(super) fn new(
        access: TaskAccess,
        signature: RelaySignature,
        request: RelayRequest,
    ) -> Self {
        let (relay_tx, relay_rx) = build_async_channel();
        let config =
            Config::load().unwrap_or_else(|error| panic!("failed to load config: {error}"));
        let model_backend: Box<dyn ModelBackend> = match config.model.backend {
            ConfigModelBackend::Deepseek => Box::new(DeepseekBackend::new()),
        };
        Self {
            access,
            signature,
            relay_tx,
            relay_rx: StdMutex::new(Some(relay_rx)),
            model_backend: StdMutex::new(model_backend),
            prompt: request.prompt,
            output: StdMutex::new(BTreeMap::new()),
            final_signal: StdMutex::new(None),
        }
    }
}

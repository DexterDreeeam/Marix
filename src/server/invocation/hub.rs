use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use marix_common::Logger;
use marix_protocol::{InvocationRequest, InvocationSignature, InvocationStatus};

use crate::invocation::Invocation;
use crate::task::TaskState;

pub struct InvocationHub {
    invocation_map: Mutex<HashMap<InvocationSignature, Invocation>>,
}

impl InvocationHub {
    pub fn new() -> Self {
        Self {
            invocation_map: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn create(
        &self,
        state: &Arc<TaskState>,
        request: InvocationRequest,
    ) -> Option<Invocation> {
        let signature = request.signature.clone();
        let invocation = Invocation::new(Arc::clone(state), request);
        let mut invocations = self
            .invocation_map
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if invocations.contains_key(&signature) {
            Logger::warning(format!(
                "invocation {} create ignored: invocation already exists",
                signature.invocation_id.0
            ));
            return None;
        }
        invocations.insert(signature, invocation.clone());
        Some(invocation)
    }

    pub fn status(&self, signature: &InvocationSignature) -> InvocationStatus {
        self.invocation_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(signature)
            .map(Invocation::status)
            .unwrap_or(InvocationStatus::Created)
    }

    pub(crate) fn content(&self, signature: &InvocationSignature) -> Option<String> {
        self.invocation_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(signature)
            .map(Invocation::content)
    }

    pub(crate) fn with<R>(
        &self,
        signature: &InvocationSignature,
        function: impl FnOnce(&Invocation) -> R,
    ) -> Option<R> {
        self.invocation_map
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(signature)
            .map(function)
    }
}

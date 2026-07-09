use std::collections::HashMap;
use std::sync::Mutex;

use marix_common::Logger;
use marix_protocol::{Actor, InvocationRequest, InvocationSignature};

use crate::invocation::Invocation;
use crate::task::TaskAccess;

pub struct InvocationHub {
    invocation_map: Mutex<HashMap<InvocationSignature, Invocation>>,
}

impl InvocationHub {
    pub fn new() -> Self {
        Self {
            invocation_map: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn create(&self, access: TaskAccess, request: InvocationRequest) -> bool {
        let signature = request.signature.clone();
        let mut invocations = self
            .invocation_map
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if invocations.contains_key(&signature) {
            Logger::warning(format!(
                "invocation {} create ignored: invocation already exists",
                &signature,
            ));
            return false;
        }
        let mut invocation = Invocation::new(access, request);
        invocation.start();
        invocations.insert(signature, invocation);
        true
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

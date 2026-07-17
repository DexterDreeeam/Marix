use std::sync::Arc;
use std::sync::OnceLock;
use std::thread;

use marix_common::{Logger, Runtime, external::uuid};
use marix_protocol::{SessionEvent, SessionMessage};

use super::{SessionRuntime, SessionState};

static SOURCE_NAME: OnceLock<String> = OnceLock::new();

pub struct Session {
    state: Arc<SessionState>,
}

impl Session {
    pub fn new(name: String) -> Self {
        let _ = SOURCE_NAME.set(name);
        let state = Arc::new(SessionState::new());
        Self { state }
    }

    pub fn session_id(&self) -> uuid::Uuid {
        self.state.session_id
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }

    pub fn start(&mut self) {
        let state = Arc::clone(&self.state);
        drop(thread::spawn(move || {
            let runtime = SessionRuntime::new(state);
            runtime.run();
        }));
    }

    pub fn dispatch(&self, event: SessionEvent) {
        if self.state.session_tx.send(event).is_err() {
            Logger::warning("core session event dispatch failed: worker stopped");
        }
    }
}

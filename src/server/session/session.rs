use std::sync::Arc;
use std::sync::OnceLock;
use std::thread;

use marix_common::Logger;
use marix_protocol::{Actor, Runtime, SessionEvent, SessionMessage};

use super::{SessionRuntime, SessionState};

static SOURCE_NAME: OnceLock<String> = OnceLock::new();

pub struct Session {
    state: Arc<SessionState>,
}

impl Session {
    pub fn new(name: String) -> Self {
        Logger::log(format!("core session '{name}' initializing"));
        let _ = SOURCE_NAME.set(name);
        let state = Arc::new(SessionState::new());
        Self { state }
    }

    pub fn package_message(event: SessionEvent) -> SessionMessage {
        SessionMessage::new(SOURCE_NAME.get().cloned().unwrap_or_default(), event)
    }
}

impl Actor<Session, SessionEvent> for Session {
    fn start(&mut self) {
        let state = Arc::clone(&self.state);
        drop(thread::spawn(move || {
            let runtime = SessionRuntime::new(state);
            runtime.run();
        }));
    }

    fn dispatch(&self, event: SessionEvent) {
        if self.state.session_tx.send(event).is_err() {
            Logger::warning("core session event dispatch failed: worker stopped");
        }
    }
}

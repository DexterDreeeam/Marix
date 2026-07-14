use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use crate::logging::{LogMessage, LogPage, LogPageQuery, LogRecord, LogSession, LoggingError};

use super::Store;

pub(super) const BATCH_SIZE: usize = 100;
pub(super) const BATCH_WINDOW: Duration = Duration::from_millis(15);
const QUEUE_CAPACITY: usize = 4096;

#[derive(Clone)]
pub(in crate::logging) struct HostStore {
    store: Arc<Store>,
    writer: StoreWriter,
}

impl HostStore {
    pub(in crate::logging) fn new(store: Store) -> Result<Self, LoggingError> {
        let store = Arc::new(store);
        let writer = StoreWriter::new(Arc::clone(&store))?;
        Ok(Self { store, writer })
    }

    pub(in crate::logging) fn record(&self, message: LogMessage) -> Result<(), LoggingError> {
        self.writer.record(message)
    }

    pub(in crate::logging) fn flush(&self) -> Result<(), LoggingError> {
        self.writer.flush()
    }

    pub(in crate::logging) fn sessions(&self) -> Result<Vec<LogSession>, LoggingError> {
        self.flush()?;
        self.store.sessions()
    }

    pub(in crate::logging) fn page(&self, query: LogPageQuery) -> Result<LogPage, LoggingError> {
        self.flush()?;
        self.store.page(query)
    }

    pub(in crate::logging) fn record_by_id(
        &self,
        id: u64,
    ) -> Result<Option<LogRecord>, LoggingError> {
        self.flush()?;
        self.store.record_by_id(id)
    }

    #[cfg(test)]
    pub(super) fn batch_commit_count(&self) -> u64 {
        self.store.batch_commit_count()
    }
}

#[derive(Clone)]
struct StoreWriter {
    sender: SyncSender<WriteRequest>,
}

impl StoreWriter {
    fn new(store: Arc<Store>) -> Result<Self, LoggingError> {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        thread::Builder::new()
            .name("marix-telemetry-writer".to_owned())
            .spawn(move || Self::run(store, receiver))
            .map_err(|error| LoggingError::Io(error.to_string()))?;
        Ok(Self { sender })
    }

    fn record(&self, message: LogMessage) -> Result<(), LoggingError> {
        let (acknowledge, result) = mpsc::sync_channel(0);
        self.sender
            .send(WriteRequest::Record {
                message,
                acknowledge,
            })
            .map_err(|error| LoggingError::Channel(error.to_string()))?;
        result
            .recv()
            .map_err(|error| LoggingError::Channel(error.to_string()))?
    }

    fn flush(&self) -> Result<(), LoggingError> {
        let (acknowledge, result) = mpsc::sync_channel(0);
        self.sender
            .send(WriteRequest::Flush { acknowledge })
            .map_err(|error| LoggingError::Channel(error.to_string()))?;
        result
            .recv()
            .map_err(|error| LoggingError::Channel(error.to_string()))?
    }
}

// -- Private -- //

enum WriteRequest {
    Record {
        message: LogMessage,
        acknowledge: SyncSender<Result<(), LoggingError>>,
    },
    Flush {
        acknowledge: SyncSender<Result<(), LoggingError>>,
    },
}

impl StoreWriter {
    fn run(store: Arc<Store>, receiver: Receiver<WriteRequest>) {
        while let Ok(request) = receiver.recv() {
            let WriteRequest::Record {
                message,
                acknowledge,
            } = request
            else {
                if let WriteRequest::Flush { acknowledge } = request {
                    let _ = acknowledge.send(Ok(()));
                }
                continue;
            };

            let mut messages = vec![message];
            let mut acknowledgements = vec![acknowledge];
            let mut flush = None;
            let deadline = Instant::now() + BATCH_WINDOW;
            while messages.len() < BATCH_SIZE {
                let timeout = deadline.saturating_duration_since(Instant::now());
                match receiver.recv_timeout(timeout) {
                    Ok(WriteRequest::Record {
                        message,
                        acknowledge,
                    }) => {
                        messages.push(message);
                        acknowledgements.push(acknowledge);
                    }
                    Ok(WriteRequest::Flush { acknowledge }) => {
                        flush = Some(acknowledge);
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout)
                    | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }

            let result = store.record_batch(&messages).map(|_ids| ());
            for acknowledge in acknowledgements {
                let _ = acknowledge.send(result.clone());
            }
            if let Some(acknowledge) = flush {
                let _ = acknowledge.send(result);
            }
        }
    }
}

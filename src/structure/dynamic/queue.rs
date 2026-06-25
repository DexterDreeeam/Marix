/// Creates a paired queue receiver and producer for one streaming channel.
pub trait DynamicQueueFactory<T> {
    fn init(&self) -> (DynamicQueue<T>, DynamicQueueProducer<T>);
}

/// Receiver-side signal for a dynamic streaming queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicQueueSignal<T> {
    Update(T),
    Complete,
    Abort,
}

/// Receiver side of a single-producer, single-consumer dynamic streaming queue.
pub struct DynamicQueue<T>;

impl<T> DynamicQueue<T> {
    /// Dequeues the next queue signal.
    ///
    /// After `Complete` is observed, later calls return `Abort`. `Abort` also
    /// represents producer drop before completion.
    pub fn dequeue(&mut self) -> DynamicQueueSignal<T> {
        panic!("not implemented")
    }
}

/// Producer side of a single-producer, single-consumer dynamic streaming queue.
pub struct DynamicQueueProducer<T>;

impl<T> DynamicQueueProducer<T> {
    /// Enqueues one update value and reports whether the receiver is still alive.
    pub fn enqueue(&mut self, _value: T) -> bool {
        panic!("not implemented")
    }

    /// Completes the queue from the producer side.
    pub fn complete(&mut self) {
        panic!("not implemented")
    }
}

pub trait DynamicQueueFactory<T> {
    fn init(&self) -> (DynamicQueue<T>, DynamicQueueProducer<T>);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicQueueSignal<T> {
    Update(T),
    Complete,
    Abort,
}

pub struct DynamicQueue<T>;

impl<T> DynamicQueue<T> {
    pub fn dequeue(&mut self) -> DynamicQueueSignal<T> {
        panic!("not implemented")
    }
}

pub struct DynamicQueueProducer<T>;

impl<T> DynamicQueueProducer<T> {
    pub fn enqueue(&mut self, _value: T) -> bool {
        panic!("not implemented")
    }

    pub fn complete(&mut self) {
        panic!("not implemented")
    }
}

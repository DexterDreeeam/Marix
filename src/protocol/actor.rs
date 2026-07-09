pub trait Actor<T, E> {
    fn start(&mut self);

    fn dispatch(&self, event: E);
}

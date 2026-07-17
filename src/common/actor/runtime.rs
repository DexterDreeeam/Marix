pub trait Runtime<E, Error> {
    fn run(&self);

    fn close(&self);

    fn dispatch(&self, event: E) -> Result<(), Error>;
}

#[allow(async_fn_in_trait)]
pub trait RuntimeAsync<E, Error> {
    async fn run(&self);

    fn close(&self);

    fn dispatch(&self, event: E) -> Result<(), Error>;
}

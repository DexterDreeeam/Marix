pub type PipeReceiveHandler<Payload> = Box<dyn Fn(Payload) -> PipeResponse + Send + 'static>;
pub type PipeCloseHandler = Box<dyn Fn() -> PipeResponse + Send + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeResponse {
    Accepted,
    Rejected(String),
}

impl PipeResponse {
    pub fn accepted() -> Self {
        Self::Accepted
    }

    pub fn rejected(reason: impl Into<String>) -> Self {
        Self::Rejected(reason.into())
    }

    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted)
    }
}

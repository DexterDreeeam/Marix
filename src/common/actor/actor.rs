use std::fmt::{Debug, Display};
use std::sync::Arc;

use crate::Logger;

use super::lifecycle::ActorStatus;
use super::runtime::Runtime;

pub type SignatureOf<A> = <A as Actor>::Signature;
pub type EventOf<A> = <A as Actor>::Event;
pub type ResultOf<A> = <A as Actor>::Result;
pub type RuntimeOf<A> = <A as Actor>::Runtime;

pub trait Actor: Send + Sync + 'static {
    type Signature: Display + Clone + Debug + Send + Sync + 'static;
    type Event: Debug + Send + 'static;
    type Result: Clone + Send + 'static;
    type Runtime: Runtime<
            Base: Actor<
                Signature = Self::Signature,
                Event = Self::Event,
                Result = Self::Result,
                Runtime = Self::Runtime,
            >,
        > + 'static;

    fn runtime(&self) -> &Arc<RuntimeOf<Self>>;

    fn spawn(&self, runtime: Arc<RuntimeOf<Self>>);

    fn signature(&self) -> &SignatureOf<Self> {
        self.runtime().signature()
    }

    fn status(&self) -> ActorStatus {
        self.runtime().lifecycle().status()
    }

    fn result(&self) -> Option<ResultOf<Self>> {
        self.runtime().lifecycle().result()
    }

    fn start(&self) {
        self.spawn(Arc::clone(self.runtime()));
    }

    fn dispatch(&self, event: EventOf<Self>) {
        if !self.runtime().lifecycle().dispatch(event) {
            Logger::warning(format!(
                "{} event dispatch failed: worker stopped",
                self.signature(),
            ));
        }
    }
}

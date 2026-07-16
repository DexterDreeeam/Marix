use std::fmt::{Debug, Display};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Logger;
use crate::external::*;
use crate::structure::AsyncReceiver;

use super::lifecycle::{ActorStatus, Lifecycle};

pub type SignatureOf<A> = <A as ActorBase>::Signature;
pub type EventOf<A> = <A as ActorBase>::Event;
pub type ResultOf<A> = <A as ActorBase>::Result;
pub type RuntimeOf<A> = <A as ActorBase>::Runtime;
pub type ActorFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
pub type ActorPrepareFuture<'a, Prepared> =
    Pin<Box<dyn Future<Output = Option<Prepared>> + Send + 'a>>;
pub type ActorEventReceiver<Event> = AsyncReceiver<Event>;
pub type ActorCloseReceiver = AsyncReceiver<()>;

pub trait ActorBase: Send + Sync + 'static {
    type Signature: Display + Clone + Debug + Send + Sync + 'static;
    type Event: Debug + Send + 'static;
    type Result: Clone + Send + 'static;
    type Runtime: ActorRuntime<
            Base: ActorBase<
                Signature = Self::Signature,
                Event = Self::Event,
                Result = Self::Result,
                Runtime = Self::Runtime,
            >,
        > + 'static;
}

pub trait Actor: ActorBase {
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

pub trait ActorRuntime: Send + Sync + 'static {
    type Base: ActorBase<Runtime: ActorRuntime<Base = Self::Base>>;
    type Prepared: Send + 'static;

    fn signature(&self) -> &SignatureOf<Self::Base>;

    fn lifecycle(&self) -> &Lifecycle<EventOf<Self::Base>, ResultOf<Self::Base>>;

    fn prepare(&self) -> ActorPrepareFuture<'_, Self::Prepared>;

    fn dispatch(&self, event: EventOf<Self::Base>);

    fn on_start(&self) {}

    fn on_finish(&self) {}

    fn status(&self) -> ActorStatus {
        self.lifecycle().status()
    }

    fn run(&self) -> ActorFuture<'_> {
        Box::pin(async move {
            let Some((event_rx, close_rx)) = self.lifecycle().begin() else {
                Logger::warning(format!(
                    "{} start ignored: already running",
                    self.signature(),
                ));
                return;
            };
            self.on_start();
            let Some(prepared) = self.prepare().await else {
                return;
            };
            if self.status().is_terminal() {
                return;
            }
            self.main(event_rx, close_rx, prepared).await;
        })
    }

    fn main<'a>(
        &'a self,
        mut event_rx: ActorEventReceiver<EventOf<Self::Base>>,
        mut close_rx: ActorCloseReceiver,
        _prepared: Self::Prepared,
    ) -> ActorFuture<'a> {
        Box::pin(async move {
            loop {
                self::tokio::select! {
                    _ = close_rx.recv() => break,
                    event = event_rx.recv() => {
                        let Some(event) = event else {
                            break;
                        };
                        self.dispatch(event);
                    }
                }
            }
        })
    }

    fn finish(&self, result: ResultOf<Self::Base>) {
        if !self.lifecycle().finish(result) {
            return;
        }
        self.on_finish();
        self.close();
    }

    fn close(&self) {
        if !self.lifecycle().close() {
            Logger::warning(format!("{} close signal failed", self.signature(),));
        }
    }
}

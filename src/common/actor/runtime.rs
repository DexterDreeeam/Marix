use std::future::Future;
use std::pin::Pin;

use crate::Logger;
use crate::external::*;
use crate::structure::AsyncReceiver;

use super::actor::{Actor, EventOf, ResultOf, SignatureOf, StatusOf};
use super::lifecycle::{ActorStatus, Lifecycle};

pub type ActorFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
pub type ActorStartFuture<'a, Prepared> =
    Pin<Box<dyn Future<Output = Option<Prepared>> + Send + 'a>>;
pub type ActorEventReceiver<Event> = AsyncReceiver<Event>;
pub type ActorCloseReceiver = AsyncReceiver<()>;

pub trait Runtime: Send + Sync + 'static {
    type Base: Actor<Runtime: Runtime<Base = Self::Base>>;
    type Prepared: Send + 'static;

    fn signature(&self) -> &SignatureOf<Self::Base>;

    fn lifecycle(&self) -> &Lifecycle<EventOf<Self::Base>, ResultOf<Self::Base>>;

    fn on_start(&self) -> ActorStartFuture<'_, Self::Prepared>;

    fn dispatch(&self, event: EventOf<Self::Base>);

    fn on_finish(&self, _result: ResultOf<Self::Base>) {}

    fn status(&self) -> StatusOf<Self::Base> {
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
            let Some(prepared) = self.on_start().await else {
                return;
            };
            if matches!(self.status(), ActorStatus::Complete(_)) {
                return;
            }
            self.event_loop(event_rx, close_rx, prepared).await;
        })
    }

    fn event_loop<'a>(
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
        if !self.lifecycle().finish(result.clone()) {
            return;
        }
        self.on_finish(result);
        self.close();
    }

    fn close(&self) {
        if !self.lifecycle().close() {
            Logger::warning(format!("{} close signal failed", self.signature(),));
        }
    }
}

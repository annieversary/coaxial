use axum::response::Response;
use generational_box::{AnyStorage, Owner, SyncStorage};
use std::{collections::HashMap, future::Future, sync::Arc};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    closure::{Closure, ClosureTrait, ClosureWrapper, IntoClosure},
    event_handlers::{EventHandler, EventHandlerWrapper},
    html::Element,
    state::{State, StateInner},
    CoaxialResponse, Output,
};

pub struct Context<S = ()> {
    uuid: u64,
    index: u64,

    state_owner: Owner<SyncStorage>,
    pub(crate) closures: HashMap<String, Arc<dyn ClosureTrait<S>>>,
    pub(crate) event_handlers: HashMap<String, Arc<dyn EventHandler>>,

    pub(crate) changes_rx: UnboundedReceiver<(u64, String)>,
    changes_tx: UnboundedSender<(u64, String)>,
}

impl<S> Context<S> {
    pub fn use_closure<P, I>(&mut self, closure: I) -> Closure
    where
        I: IntoClosure<P, S> + Send + Sync + 'static,
        P: Send + Sync + 'static,
        ClosureWrapper<I, P>: ClosureTrait<S>,
    {
        self.index += 1;
        let id = format!("{}-{}", self.uuid, self.index);
        let closure: ClosureWrapper<I, P> = <I as IntoClosure<P, S>>::wrap(closure);
        self.closures.insert(id.clone(), Arc::new(closure));

        Closure { id }
    }

    #[track_caller]
    pub fn use_state<T: Send + Sync>(&mut self, value: T) -> State<T> {
        self.index += 1;
        State {
            inner: self.state_owner.insert_with_caller(
                StateInner {
                    value,
                    changes_tx: self.changes_tx.clone(),
                },
                #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                std::panic::Location::caller(),
            ),
            id: self.index + self.uuid,
        }
    }

    // TODO ideally, we would store a function that takes a type that impls Deserialize
    // idk how to do it with multiple functions tho
    pub fn on<F, Fut, P>(&mut self, name: impl ToString, closure: F)
    where
        F: Fn(P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + Sync + 'static,
        P: serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        self.event_handlers.insert(
            name.to_string(),
            Arc::new(EventHandlerWrapper::new(closure)),
        );
    }

    pub fn with(self, element: Element) -> CoaxialResponse<S> {
        Response::new(Output {
            element,
            context: self,
        })
    }
}

impl<S> Default for Context<S> {
    fn default() -> Self {
        let (changes_tx, changes_rx) = unbounded_channel();

        Self {
            // TODO generate something random
            uuid: 100000,
            index: 0,
            state_owner: <SyncStorage as AnyStorage>::owner(),
            closures: Default::default(),
            event_handlers: Default::default(),

            changes_rx,
            changes_tx,
        }
    }
}

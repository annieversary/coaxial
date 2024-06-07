use axum::response::Response;
use generational_box::{AnyStorage, Owner, SyncStorage};
use std::{collections::HashMap, future::Future, sync::Arc};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    closure::{AsyncFn, Closure},
    html::Element,
    state::{State, StateInner},
    CoaxialResponse, Output,
};

pub struct Context {
    uuid: u64,
    index: u64,

    state_owner: Owner<SyncStorage>,
    pub(crate) closures: HashMap<String, Arc<dyn AsyncFn<()>>>,
    // TODO idk how to erase the param type
    // maybe we just do serde_json::Value directly
    pub(crate) event_handlers: HashMap<String, Arc<dyn AsyncFn<(serde_json::Value,)>>>,

    pub(crate) changes_rx: UnboundedReceiver<(u64, String)>,
    changes_tx: UnboundedSender<(u64, String)>,
}

impl Context {
    pub fn use_closure<F, Fut>(&mut self, closure: F) -> Closure
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.index += 1;
        let id = format!("{}-{}", self.uuid, self.index);
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

    pub fn on<F, Fut>(&mut self, name: impl ToString, closure: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.event_handlers
            .insert(name.to_string(), Arc::new(closure));
    }

    pub fn with(self, element: Element) -> CoaxialResponse {
        Response::new(Output {
            element,
            context: self,
        })
    }
}

impl Default for Context {
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

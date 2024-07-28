use axum::response::Response;
use generational_box::{AnyStorage, Owner, SyncStorage};
use rand::{rngs::StdRng, SeedableRng};
use serde::de::DeserializeOwned;
use std::{
    collections::HashMap,
    fmt::{Display, Write},
    future::Future,
    sync::Arc,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    closure::{Closure, ClosureTrait, ClosureWrapper, Closures, IntoClosure},
    event_handlers::{EventHandler, EventHandlerWrapper},
    html::{Content, Element},
    random_id::RandomId,
    state::{AnyState, State, StateInner},
    CoaxialResponse, Output,
};

pub struct Context<S = ()> {
    pub(crate) rng: StdRng,
    rng_seed: u64,

    state_owner: Owner<SyncStorage>,
    pub(crate) states: HashMap<RandomId, Arc<dyn AnyState>>,
    // TODO we might want to change closures to have RandomIds as keys
    pub(crate) closures: Closures<S>,
    pub(crate) event_handlers: HashMap<String, Arc<dyn EventHandler>>,

    pub(crate) changes_rx: UnboundedReceiver<(RandomId, String)>,
    changes_tx: UnboundedSender<(RandomId, String)>,

    pub(crate) closure_call_rx: UnboundedReceiver<Closure>,
    closure_call_tx: UnboundedSender<Closure>,
}

impl<S> Context<S> {
    pub(crate) fn new(seed: u64) -> Self {
        let (changes_tx, changes_rx) = unbounded_channel();
        let (closure_call_tx, closure_call_rx) = unbounded_channel();

        let rng = StdRng::seed_from_u64(seed);

        Self {
            rng,
            rng_seed: seed,

            state_owner: <SyncStorage as AnyStorage>::owner(),
            states: Default::default(),
            closures: Default::default(),
            event_handlers: Default::default(),

            changes_rx,
            changes_tx,
            closure_call_rx,
            closure_call_tx,
        }
    }

    pub fn use_closure<P, I>(&mut self, closure: I) -> Closure
    where
        I: IntoClosure<P, S> + Send + Sync + 'static,
        P: Send + Sync + 'static,
        ClosureWrapper<I, P>: ClosureTrait<S>,
    {
        let id = RandomId::from_rng(&mut self.rng);

        let closure: ClosureWrapper<I, P> = <I as IntoClosure<P, S>>::wrap(closure);
        self.closures.insert(id, Arc::new(closure));

        Closure {
            id,
            closure_call_tx: self.closure_call_tx.clone(),
        }
    }

    #[track_caller]
    pub fn use_state<T: DeserializeOwned + Display + Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> State<T> {
        let id = RandomId::from_rng(&mut self.rng);
        let state = State {
            inner: self.state_owner.insert_with_caller(
                StateInner {
                    value,
                    changes_tx: self.changes_tx.clone(),
                },
                #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                std::panic::Location::caller(),
            ),
            id,
        };

        self.states.insert(state.id, Arc::new(state));

        state
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

    /// Returns an Element containing an HTML `<script>` tag containing the adapter JS code.
    pub(crate) fn adapter_script_element(&self, reactive_scripts: &str) -> Element {
        let mut script = include_str!("base.js")
            .to_string()
            .replace("__internal__coaxialSeed", &self.rng_seed.to_string());

        for (name, handler) in &self.event_handlers {
            script.push_str("document.addEventListener('");
            script.push_str(name);
            script.push_str("', params=>{params={");

            // NOTE: this serves two puposes:
            // 1. events are big objects with lots of fields, so we only wanna send the ones we care about over the wire
            // 2. serialization of events is wonky, and a lot of times fields are not set correctly
            for field in handler.param_fields() {
                script.push_str(field);
                script.push_str(": params.");
                script.push_str(field);
                script.push(',');
            }

            script.push_str("};if (window.Coaxial) window.Coaxial.onEvent('");
            script.push_str(name);
            script.push_str("', params);});");
        }

        script
            .write_fmt(format_args!(
                "document.addEventListener(\"DOMContentLoaded\", () => {{ {} }});",
                reactive_scripts
            ))
            .unwrap();

        crate::html::script(
            Content::Raw(html_escape::encode_script(&script).to_string()),
            Default::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u32_state_in_closures() {
        let mut ctx = Context::<()>::new(0);

        let state = ctx.use_state(0u32);

        ctx.use_closure(move || async move {
            state.set(1);
        });

        // TODO run the closure. assert that it gets set to the correct thing
    }

    #[test]
    fn test_string_state_in_closures() {
        let mut ctx = Context::<()>::new(0);

        let state = ctx.use_state("my string".to_string());

        ctx.use_closure(move || async move {
            state.set("other string".to_string());
        });

        // TODO run the closure. assert that it gets set to the correct thing
        // we will need a way to call the closure directly, without the having the websocket connection set up
        // probably by passing in an empty state and a default Parts as a test
    }
}

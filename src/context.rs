use axum::response::Response;
use generational_box::{AnyStorage, Owner, SyncStorage};
use rand::{rngs::StdRng, SeedableRng};
use serde::de::DeserializeOwned;
use std::{
    collections::HashMap,
    fmt::{Display, Write},
    future::Future,
    pin::Pin,
    sync::Arc,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    closure::{Closure, ClosureTrait, ClosureWrapper, Closures, IntoClosure},
    computed::{ComputedState, StateGetter},
    event_handlers::{EventHandler, EventHandlerWrapper},
    html::{Content, ContentValue, Element},
    random_id::RandomId,
    state::{AnyState, State, StateInner},
    CoaxialResponse, Output,
};

type OnChangeHandler = Arc<dyn Fn() + 'static + Send + Sync>;
type OnChangeHandlerAsync =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send + Sync>> + Send + Sync>;

pub struct Context<S = ()> {
    pub(crate) rng: StdRng,
    rng_seed: u64,

    state_owner: Owner<SyncStorage>,
    pub(crate) states: HashMap<RandomId, Arc<dyn AnyState>>,
    pub(crate) closures: Closures<S>,
    pub(crate) event_handlers: HashMap<String, Arc<dyn EventHandler>>,

    pub(crate) on_change_handler: HashMap<RandomId, Vec<OnChangeHandler>>,
    pub(crate) on_change_handler_async: HashMap<RandomId, Vec<OnChangeHandlerAsync>>,

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

            on_change_handler: Default::default(),
            on_change_handler_async: Default::default(),

            changes_rx,
            changes_tx,
            closure_call_rx,
            closure_call_tx,
        }
    }

    #[track_caller]
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
            closure_call_tx: self.state_owner.insert_with_caller(
                self.closure_call_tx.clone(),
                #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                std::panic::Location::caller(),
            ),
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

    // TODO move most of the contents of this function to an object defined in the computed file
    pub fn use_computed<O, I, F>(&mut self, states: I, compute: F) -> ComputedState<O>
    where
        O: DeserializeOwned + Display + Send + Sync + 'static,
        I: StateGetter + Send + Sync + 'static,
        F: Fn(<I as StateGetter>::Output) -> O + Send + Sync + 'static,
    {
        let state = self.use_state(compute(states.get()));

        let compute = Arc::new(compute);
        for id in states.id_list() {
            let compute = compute.clone();
            let states = states.clone();
            let on_change_listener = move || {
                state.set(compute(states.get()));
            };

            if let Some(value) = self.on_change_handler.get_mut(&id) {
                value.push(Arc::new(on_change_listener));
            } else {
                self.on_change_handler
                    .insert(id, vec![Arc::new(on_change_listener)]);
            }
        }

        ComputedState(state)
    }

    pub async fn use_computed_async<O, I, F, FUT>(&mut self, states: I, compute: F) -> State<O>
    where
        O: DeserializeOwned + Display + Send + Sync + 'static,
        I: StateGetter,
        F: Fn(<I as StateGetter>::Output) -> FUT + Send + Sync + 'static,
        FUT: Future<Output = O> + Send + Sync + 'static,
    {
        let state = self.use_state(compute(states.get()).await);

        let compute = Arc::new(compute);
        for id in states.id_list() {
            let compute = compute.clone();
            let states = states.clone();
            let on_change_listener: OnChangeHandlerAsync = Arc::new(move || {
                let compute = compute.clone();
                let states = states.clone();
                Box::pin(async move {
                    state.set(compute(states.get()).await);
                })
            });

            if let Some(value) = self.on_change_handler_async.get_mut(&id) {
                value.push(on_change_listener);
            } else {
                self.on_change_handler_async
                    .insert(id, vec![on_change_listener]);
            }
        }

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
            Content::Value(ContentValue::Raw(
                html_escape::encode_script(&script).to_string(),
            )),
            Default::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{request::Parts, Request};

    use super::*;

    fn parts() -> Parts {
        let req = Request::new(());
        let (parts, _) = req.into_parts();
        parts
    }

    #[tokio::test]
    async fn test_u32_state_in_closures() {
        let mut ctx = Context::<()>::new(0);

        let state = ctx.use_state(0u32);

        let closure = ctx.use_closure(move || async move {
            state.set(1);
        });

        // we run the closure manually, not by calling call
        // call relies on the websocket loop to be running
        let func = ctx.closures.get(&closure.id).unwrap();

        func.call(parts(), ()).await;

        assert_eq!(1, state.get());
    }

    #[tokio::test]
    async fn test_string_state_in_closures() {
        let mut ctx = Context::<()>::new(0);

        let state = ctx.use_state("my string".to_string());

        let closure = ctx.use_closure(move || async move {
            state.set("other string".to_string());
        });

        // we run the closure manually, not by calling call
        // call relies on the websocket loop to be running
        let func = ctx.closures.get(&closure.id).unwrap();

        func.call(parts(), ()).await;

        assert_eq!("other string", state.get());
    }
}

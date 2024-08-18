use axum::response::Response;
use generational_box::{AnyStorage, Owner, SyncStorage};
use rand::{rngs::StdRng, SeedableRng};
use serde::de::DeserializeOwned;
use std::{
    fmt::{Display, Write},
    future::Future,
    panic::Location,
    sync::Arc,
};

use crate::{
    closures::{Closure, ClosureInner, ClosureTrait, ClosureWrapper, Closures, IntoClosure},
    computed::{ComputedState, ComputedStates, InitialValue, StateGetter},
    event_handlers::Events,
    html::{Content, ContentValue, Element},
    random_id::RandomId,
    state::{State, StateInner, States},
    CoaxialResponse, Output,
};

pub struct Context<S = ()> {
    pub(crate) rng: StdRng,
    rng_seed: u64,

    in_websocket: bool,

    state_owner: Owner<SyncStorage>,

    pub(crate) states: States,
    pub(crate) events: Events,
    pub(crate) closures: Closures<S>,
    pub(crate) computed_states: ComputedStates,
}

impl<S> Context<S> {
    pub(crate) fn new(seed: u64, in_websocket: bool) -> Self {
        let rng = StdRng::seed_from_u64(seed);

        Self {
            rng,
            rng_seed: seed,
            in_websocket,

            state_owner: <SyncStorage as AnyStorage>::owner(),

            states: Default::default(),
            events: Default::default(),
            closures: Default::default(),
            computed_states: Default::default(),
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
            inner: self.state_owner.insert_with_caller(
                ClosureInner {
                    closure_call_tx: self.closures.call_tx.clone(),
                },
                #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                std::panic::Location::caller(),
            ),
        }
    }

    pub fn use_state_inner<T: DeserializeOwned + Display + Send + Sync + 'static>(
        &mut self,
        value: T,
        #[cfg(any(debug_assertions, feature = "debug_ownership"))] caller: &'static Location<
            'static,
        >,
    ) -> State<T> {
        let id = RandomId::from_rng(&mut self.rng);
        let state = State {
            inner: self.state_owner.insert_with_caller(
                StateInner {
                    value,
                    changes_tx: self.states.changes_tx.clone(),
                },
                #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                caller,
            ),
            id,
        };

        self.states.insert(state.id, Arc::new(state));

        state
    }

    #[track_caller]
    pub fn use_state<T: DeserializeOwned + Display + Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> State<T> {
        self.use_state_inner(
            value,
            #[cfg(any(debug_assertions, feature = "debug_ownership"))]
            std::panic::Location::caller(),
        )
    }

    pub fn use_computed<O, I, F>(&mut self, states: I, compute: F) -> ComputedState<O>
    where
        O: DeserializeOwned + Display + Send + Sync + 'static,
        I: StateGetter + Send + Sync + 'static,
        F: Fn(<I as StateGetter>::Output) -> O + Send + Sync + 'static,
    {
        let state = self.use_state_inner(
            compute(states.get()),
            #[cfg(any(debug_assertions, feature = "debug_ownership"))]
            std::panic::Location::caller(),
        );

        self.computed_states.add_computed(state, states, compute)
    }

    pub fn use_computed_with<O, I, F>(
        &mut self,
        states: I,
        compute: F,
        initial: InitialValue<O>,
    ) -> ComputedState<O>
    where
        O: DeserializeOwned + Display + Send + Sync + 'static,
        I: StateGetter + Send + Sync + 'static,
        F: Fn(<I as StateGetter>::Output) -> O + Send + Sync + 'static,
    {
        let initial = match initial {
            InitialValue::Value(value) => value,
            // it's a blocking function, so we can't run it in the background.
            // we just recompute and ignore the provided value
            InitialValue::ValueAndCompute(_value) => compute(states.get()),
        };

        let state = self.use_state_inner(
            initial,
            #[cfg(any(debug_assertions, feature = "debug_ownership"))]
            std::panic::Location::caller(),
        );

        self.computed_states.add_computed(state, states, compute)
    }

    pub async fn use_computed_async<O, I, F, FUT>(
        &mut self,
        states: I,
        compute: F,
    ) -> ComputedState<O>
    where
        O: DeserializeOwned + Display + Send + Sync + 'static,
        I: StateGetter,
        F: Fn(<I as StateGetter>::Output) -> FUT + Send + Sync + 'static,
        FUT: Future<Output = O> + Send + Sync + 'static,
    {
        // no tracking caller cause this function is async and track_caller doesn't work on async functions yet
        // https://github.com/rust-lang/rust/issues/110011
        let state = self.use_state(compute(states.get()).await);

        self.computed_states
            .add_computed_async(state, states, compute, false)
    }

    #[track_caller]
    pub fn use_computed_async_with<O, I, F, FUT>(
        &mut self,
        states: I,
        compute: F,
        initial: InitialValue<O>,
    ) -> ComputedState<O>
    where
        O: DeserializeOwned + Display + Send + Sync + 'static,
        I: StateGetter,
        F: Fn(<I as StateGetter>::Output) -> FUT + Send + Sync + 'static,
        FUT: Future<Output = O> + Send + Sync + 'static,
    {
        let mut needs_recompute = false;
        let initial = match initial {
            InitialValue::Value(value) => value,
            InitialValue::ValueAndCompute(value) => {
                needs_recompute = true;
                value
            }
        };

        let state = self.use_state_inner(
            initial,
            #[cfg(any(debug_assertions, feature = "debug_ownership"))]
            std::panic::Location::caller(),
        );

        self.computed_states.add_computed_async(
            state,
            states,
            compute,
            needs_recompute && self.in_websocket,
        )
    }

    pub fn on_client_event<F, Fut, P>(&mut self, name: impl ToString, closure: F)
    where
        F: Fn(P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + Sync + 'static,
        P: serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        self.events.add(name.to_string(), closure);
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

        for (name, fields) in self.events.list() {
            script.push_str("document.addEventListener('");
            script.push_str(name);
            script.push_str("', params=>{params={");

            // NOTE: this serves two puposes:
            // 1. events are big objects with lots of fields, so we only wanna send the ones we care about over the wire
            // 2. serialization of events is wonky, and a lot of times fields are not set correctly
            for field in fields {
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

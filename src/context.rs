use axum::response::Response;
use futures::StreamExt;
use futures_signals::signal::SignalExt;
use generational_box::{AnyStorage, Owner, SyncStorage};
use rand::{rngs::StdRng, SeedableRng};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Write, future::Future, sync::Arc};

use crate::{
    closures::{Closure, ClosureInner, ClosureTrait, ClosureWrapper, Closures, IntoClosure},
    computed::{ComputedState, ComputedStates, InitialValue, StateGetter},
    events::Events,
    html::{Content, ContentValue, Element},
    random_id::RandomId,
    states::{State, States},
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

    fn use_state_inner<T: DeserializeOwned + Serialize + Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> State<T> {
        let id = RandomId::from_rng(&mut self.rng);

        let state = State::new(value, id);
        let stream = state
            .inner
            .signal_ref(move |value| {
                (
                    id,
                    serde_json::to_value(value).expect("could not convert to Value"),
                )
            })
            .to_stream()
            .boxed();

        self.states.insert(state.id, Arc::new(state.clone()));
        self.states.streams.push(stream);

        state
    }

    pub fn use_state<T: DeserializeOwned + Serialize + Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> State<T> {
        self.use_state_inner(value)
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

use axum::response::Response;
use generational_box::{AnyStorage, Owner, SyncStorage};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, fmt::Display, future::Future, sync::Arc};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    closure::{Closure, ClosureTrait, ClosureWrapper, IntoClosure},
    event_handlers::{EventHandler, EventHandlerWrapper},
    html::{Content, Element},
    random_id,
    state::{AnyState, State, StateId, StateInner},
    CoaxialResponse, Output,
};

pub struct Context<S = ()> {
    uuid: u64,
    index: u64,

    pub(crate) rng: StdRng,
    rng_seed: u64,

    state_owner: Owner<SyncStorage>,
    pub(crate) states: HashMap<StateId, Arc<dyn AnyState>>,
    pub(crate) closures: HashMap<String, Arc<dyn ClosureTrait<S>>>,
    pub(crate) event_handlers: HashMap<String, Arc<dyn EventHandler>>,

    pub(crate) changes_rx: UnboundedReceiver<(StateId, String)>,
    changes_tx: UnboundedSender<(StateId, String)>,
}

impl<S> Context<S> {
    pub(crate) fn new(seed: u64) -> Self {
        let (changes_tx, changes_rx) = unbounded_channel();

        let mut rng = StdRng::seed_from_u64(seed);

        Self {
            uuid: rng.gen(),
            index: 0,

            rng,
            rng_seed: seed,

            state_owner: <SyncStorage as AnyStorage>::owner(),
            states: Default::default(),
            closures: Default::default(),
            event_handlers: Default::default(),

            changes_rx,
            changes_tx,
        }
    }

    pub fn use_closure<P, I>(&mut self, closure: I) -> Closure
    where
        I: IntoClosure<P, S> + Send + Sync + 'static,
        P: Send + Sync + 'static,
        ClosureWrapper<I, P>: ClosureTrait<S>,
    {
        self.index += 1;
        let id = random_id(&mut self.rng);

        let closure: ClosureWrapper<I, P> = <I as IntoClosure<P, S>>::wrap(closure);
        self.closures.insert(id.clone(), Arc::new(closure));

        Closure { id }
    }

    #[track_caller]
    pub fn use_state<T: DeserializeOwned + Display + Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> State<T> {
        self.index += 1;
        let state = State {
            inner: self.state_owner.insert_with_caller(
                StateInner {
                    value,
                    changes_tx: self.changes_tx.clone(),
                },
                #[cfg(any(debug_assertions, feature = "debug_ownership"))]
                std::panic::Location::caller(),
            ),
            id: StateId(self.index, self.uuid),
        };

        self.states.insert(state.id, Arc::new(state.clone()));

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
    pub(crate) fn adapter_script_element(&self) -> Element {
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

        crate::html::script(
            Content::Raw(html_escape::encode_script(&script).to_string()),
            Default::default(),
        )
    }
}

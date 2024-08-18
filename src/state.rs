use generational_box::{GenerationalBox, SyncStorage};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{collections::HashMap, fmt::Display, sync::Arc};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::random_id::RandomId;

pub(crate) struct States {
    states: HashMap<RandomId, Arc<dyn AnyState>>,

    pub(crate) changes_rx: UnboundedReceiver<(RandomId, String)>,
    pub(crate) changes_tx: UnboundedSender<(RandomId, String)>,
}

impl States {
    pub(crate) fn insert(&mut self, id: RandomId, state: Arc<dyn AnyState>) {
        self.states.insert(id, state);
    }

    pub(crate) fn set(&self, id: RandomId, value: Value) {
        let Some(state) = self.states.get(&id) else {
            // TODO return an error
            panic!("state not found");
        };
        state.set_value(value);
    }
}

impl Default for States {
    fn default() -> Self {
        let (changes_tx, changes_rx) = unbounded_channel();
        Self {
            states: Default::default(),
            changes_rx,
            changes_tx,
        }
    }
}

pub struct State<T: 'static> {
    pub(crate) inner: GenerationalBox<StateInner<T>, SyncStorage>,
    pub(crate) id: RandomId,
}

// we implement Copy and Clone instead of deriving them, cause we dont need the
// `T: Clone` bound
impl<T: 'static> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: 'static> Copy for State<T> {}

pub(crate) struct StateInner<T: 'static> {
    pub(crate) value: T,
    pub(crate) changes_tx: UnboundedSender<(RandomId, String)>,
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    pub fn get(&self) -> T {
        self.inner.read().value.clone()
    }
}

impl<T: Display + Send + Sync + 'static> State<T> {
    pub fn set(&self, value: T) {
        let mut w = self.inner.write();
        w.changes_tx.send((self.id, format!("{value}"))).unwrap();
        w.value = value;
    }
}

pub trait AnyState: Send + Sync + 'static {
    fn set_value(&self, value: serde_json::Value);
}

impl<T: DeserializeOwned + Display + Send + Sync + 'static> AnyState for State<T> {
    fn set_value(&self, value: serde_json::Value) {
        // numbers arrive as strings, so the from_value later doesn't work
        // we manually test inside the string.
        // if it succeeds we set the value, and if it fails we ignore and try the normal deserialize
        if let serde_json::Value::String(s) = &value {
            if let Ok(value) = serde_json::from_str::<T>(s) {
                self.set(value);
                return;
            }
        }

        let value: T = serde_json::from_value(value).unwrap();
        self.set(value);
    }
}

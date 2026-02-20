use futures::stream::{SelectAll, Stream};
use futures_signals::signal::Mutable;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

use crate::random_id::RandomId;

#[derive(Default)]
pub(crate) struct States {
    // TODO do we actually need to store this?
    // i think we do, so we can set things from the client
    pub(crate) states: HashMap<RandomId, Arc<dyn AnyState>>,

    pub(crate) streams:
        SelectAll<std::pin::Pin<Box<dyn Stream<Item = (RandomId, serde_json::Value)> + Send>>>,
}

impl States {
    pub(crate) fn insert(&mut self, id: RandomId, state: Arc<dyn AnyState>) {
        self.states.insert(id, state);
    }
}

pub struct State<T> {
    pub(crate) inner: Mutable<T>,
    pub(crate) id: RandomId,
}
impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            id: self.id,
        }
    }
}

impl<T> State<T> {
    pub(crate) fn new(value: T, id: RandomId) -> Self {
        Self {
            inner: Mutable::new(value),
            id,
        }
    }

    pub fn set(&mut self, value: T) {
        self.inner.set(value);
    }

    pub fn lock_mut(&self) -> futures_signals::signal::MutableLockMut<T> {
        self.inner.lock_mut()
    }

    pub fn lock_ref(&self) -> futures_signals::signal::MutableLockRef<T> {
        self.inner.lock_ref()
    }

    pub fn replace(&self, value: T) -> T {
        self.inner.replace(value)
    }

    pub fn replace_with<F>(&self, f: F) -> T
    where
        F: FnOnce(&mut T) -> T,
    {
        self.inner.replace_with(f)
    }
}

impl<T: Copy> State<T> {
    pub fn get(&self) -> T {
        self.inner.get()
    }
}
impl<T: Clone> State<T> {
    pub fn get_cloned(&self) -> T {
        self.inner.get_cloned()
    }
}

pub trait AnyState: Send + Sync + 'static {
    fn set_value(&self, value: Value);
}

impl<T: DeserializeOwned + Send + Sync + 'static> AnyState for State<T> {
    fn set_value(&self, value: serde_json::Value) {
        // numbers arrive as strings, so the from_value later doesn't work
        // we manually test inside the string.
        // if it succeeds we set the value, and if it fails we ignore and try the normal deserialize
        if let serde_json::Value::String(s) = &value {
            if let Ok(value) = serde_json::from_str::<T>(s) {
                let mut lock = self.inner.lock_mut();
                *lock = value;
                return;
            }
        }

        let value: T = serde_json::from_value(value).unwrap();
        let mut lock = self.inner.lock_mut();
        *lock = value;
    }
}

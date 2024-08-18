use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use serde_json::Value;
use tokio::task::JoinSet;

use crate::helpers;

#[derive(Default)]
pub(crate) struct Events {
    // TODO this should be a hashmap to a list
    handlers: HashMap<String, Arc<dyn EventHandler>>,

    join_set: JoinSet<()>,
}

impl Events {
    // TODO ideally, we would store a function that takes a type that impls Deserialize
    // idk how to do it with multiple functions tho
    pub(crate) fn add<F, Fut, P>(&mut self, name: String, closure: F)
    where
        F: Fn(P) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + Sync + 'static,
        P: serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        self.handlers
            .insert(name, Arc::new(EventHandlerWrapper::new(closure)));
    }

    pub(crate) fn handle(&mut self, name: String, params: Value) {
        let Some(event) = self.handlers.get(&name) else {
            return;
        };

        let event = event.clone();
        self.join_set.spawn(async move { event.call(params).await });
    }

    pub(crate) fn list(&self) -> impl Iterator<Item = (&str, &[&str])> {
        self.handlers
            .iter()
            .map(|(name, handler)| (name.as_str(), handler.param_fields()))
    }
}

trait EventHandler: Send + Sync {
    fn call(&self, params: serde_json::Value)
        -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

    /// List of fields that are set in the type that is passed to the handler
    fn param_fields(&self) -> &'static [&'static str];
}

struct EventHandlerWrapper<P, F, T> {
    func: T,
    _phantom: PhantomData<(P, F)>,
}

impl<P, F, T> EventHandlerWrapper<P, F, T> {
    pub(crate) fn new(func: T) -> Self {
        Self {
            func,
            _phantom: PhantomData,
        }
    }
}

impl<T: Send + Sync, F, P> EventHandler for EventHandlerWrapper<P, F, T>
where
    T: Fn(P) -> F,
    F: Future<Output = ()> + 'static + Send + Sync,
    P: serde::de::DeserializeOwned + Send + Sync,
{
    fn call(
        &self,
        params: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        let p: P = serde_json::from_value(params).unwrap();
        Box::pin((self.func)(p))
    }

    fn param_fields(&self) -> &'static [&'static str] {
        helpers::struct_fields::<'_, P>()
    }
}

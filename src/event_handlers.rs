use std::{future::Future, marker::PhantomData, pin::Pin};

use crate::helpers;

pub(crate) trait EventHandler: Send + Sync {
    fn call(&self, params: serde_json::Value) -> Pin<Box<dyn Future<Output = ()> + 'static>>;

    /// List of fields that are set in the type that is passed to the handler
    fn param_fields(&self) -> &'static [&'static str];
}

pub(crate) struct EventHandlerWrapper<P, F, T> {
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
    fn call(&self, params: serde_json::Value) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
        let p: P = serde_json::from_value(params).unwrap();
        Box::pin((self.func)(p))
    }

    fn param_fields(&self) -> &'static [&'static str] {
        helpers::struct_fields::<'_, P>()
    }
}

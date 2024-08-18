use std::{
    collections::{HashMap, HashSet},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
};

use serde_json::Value;
use tokio::task::JoinSet;

use crate::helpers;

#[derive(Default)]
pub(crate) struct Events {
    events: HashMap<String, Event>,

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
        if let Some(event) = self.events.get_mut(&name) {
            let wrapper = EventHandlerWrapper::new(closure);

            if let Some(params) = helpers::struct_fields::<'_, P>() {
                for param in params {
                    event.params.insert(param);
                }
            }

            event.handlers.push(Arc::new(wrapper));
        } else {
            let wrapper = EventHandlerWrapper::new(closure);

            let params = helpers::struct_fields::<'_, P>().unwrap_or_default();
            let params = HashSet::from_iter(params.iter().cloned());

            let event = Event {
                handlers: vec![Arc::new(wrapper)],
                params,
            };

            self.events.insert(name, event);
        }
    }

    pub(crate) fn handle(&mut self, name: String, params: Value) {
        let Some(event) = self.events.get(&name) else {
            return;
        };

        for handler in &event.handlers {
            let handler = handler.clone();
            let params = params.clone();
            self.join_set
                .spawn(async move { handler.call(params).await });
        }
    }

    pub(crate) fn list(&self) -> impl Iterator<Item = (&str, impl Iterator<Item = &str>)> {
        self.events
            .iter()
            .map(|(name, event)| (name.as_str(), event.params.iter().cloned()))
    }
}

struct Event {
    handlers: Vec<Arc<dyn EventHandler>>,
    params: HashSet<&'static str>,
}

trait EventHandler: Send + Sync {
    fn call(&self, params: serde_json::Value)
        -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
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
}

// TODO add tests

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use crate::context::Context;

    #[tokio::test]
    async fn test_can_handle_events() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state("not clicked :(".to_string());

        ctx.on_client_event("click", move |_event: Value| async move {
            state.set("clicked :D".to_string());
        });

        ctx.events.handle("click".to_string(), Value::Null);
        ctx.events.join_set.join_next().await.unwrap().unwrap();

        assert_eq!("clicked :D", state.get());
    }

    #[tokio::test]
    async fn test_can_list_events() {
        let mut ctx = Context::<()>::new(0, true);

        #[derive(serde::Deserialize)]
        struct First {
            _field_one: i32,
        }
        ctx.on_client_event("first", move |_event: First| async move {});

        #[derive(serde::Deserialize)]
        struct SecondOne {
            _second_one: i32,
        }
        ctx.on_client_event("second", move |_event: SecondOne| async move {});

        #[derive(serde::Deserialize)]
        struct SecondTwo {
            _second_two: i32,
        }
        ctx.on_client_event("second", move |_event: SecondTwo| async move {});

        let mut list = ctx
            .events
            .list()
            .map(|(event, params)| {
                (event, {
                    let mut params = params.collect::<Vec<_>>();
                    params.sort();
                    params
                })
            })
            .collect::<Vec<_>>();
        list.sort_by_key(|(event, _)| *event);

        assert_eq!(
            vec![
                ("first", vec!["_field_one"]),
                ("second", vec!["_second_one", "_second_two"]),
            ],
            list
        )
    }
}

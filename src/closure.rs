use axum::{extract::FromRequestParts, http::request::Parts};
use generational_box::{GenerationalBox, SyncStorage};
use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin, sync::Arc};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinSet,
};

use crate::random_id::RandomId;

pub(crate) struct Closures<S> {
    closures: HashMap<RandomId, Arc<dyn ClosureTrait<S>>>,

    pub(crate) call_rx: UnboundedReceiver<RandomId>,
    pub(crate) call_tx: UnboundedSender<RandomId>,

    join_set: JoinSet<()>,
}

impl<S> Closures<S> {
    pub(crate) fn insert(&mut self, id: RandomId, closure: Arc<dyn ClosureTrait<S>>) {
        self.closures.insert(id, closure);
    }
}

impl<S: Clone + Send + 'static> Closures<S> {
    pub(crate) fn run(&mut self, id: RandomId, parts: &Parts, state: &S) {
        let Some(closure) = self.closures.get(&id) else {
            // this is a fatal error
            return;
        };

        let closure = closure.clone();
        let parts = parts.clone();
        let state = state.clone();

        self.join_set
            .spawn(async move { closure.call(parts, state).await });
    }
}

impl<S> Default for Closures<S> {
    fn default() -> Self {
        let (call_tx, call_rx) = unbounded_channel();

        Self {
            closures: Default::default(),
            call_rx,
            call_tx,
            join_set: Default::default(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Closure {
    pub(crate) id: RandomId,
    pub(crate) inner: GenerationalBox<ClosureInner, SyncStorage>,
}

pub(crate) struct ClosureInner {
    pub(crate) closure_call_tx: UnboundedSender<RandomId>,
}

impl Closure {
    /// Queues the function to be run
    ///
    /// Note: this doesn't call the closure immediately.
    /// Keep in mind, the closure will not be run until the websocket connection has been established.
    pub fn call(&self) {
        self.inner.read().closure_call_tx.send(self.id).unwrap();
    }
}

/// Trait used to type-erase all closures, so they can be stored in the same HashMap
pub trait ClosureTrait<S>: Send + Sync {
    fn call<'a>(&'a self, parts: Parts, state: S) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

impl<S, F, Fut> ClosureTrait<S> for ClosureWrapper<F, ()>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = ()> + Send + Sync + 'static,
{
    fn call(&self, _parts: Parts, _state: S) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        Box::pin((self.func)())
    }
}

macro_rules! impl_closure_trait {
    (
        $($ty:ident),*
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<S, F, Fut, $($ty,)*> ClosureTrait<S> for ClosureWrapper<F, ($($ty,)*)>
        where
            F: Fn($($ty,)*) -> Fut + Send + Sync,
            Fut: Future<Output = ()> + Send + Sync + 'static,
        $( $ty: FromRequestParts<S> + Send + Sync, )*
            S: Send + Sync + 'static
        {
            fn call<'a>(
                &'a self,
                mut parts: Parts,
                state: S,
            ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
                Box::pin(async move {
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
                            Ok(value) => value,
                            Err(_rejection) => todo!("rejections aren't handled yet"),
                        };
                    )*

                    (self.func)($($ty,)*).await
                })
            }
        }
    };
}

/// Wrapper type that actually implements `ClosureTrait`
pub struct ClosureWrapper<T, P> {
    func: T,
    _phantom: PhantomData<P>,
}

pub trait IntoClosure<P, S> {
    fn wrap<F>(func: F) -> ClosureWrapper<F, P> {
        ClosureWrapper {
            func,
            _phantom: Default::default(),
        }
    }
}

impl<S, T, F> IntoClosure<(), S> for T
where
    T: Fn() -> F,
    F: Future<Output = ()> + 'static,
{
}

macro_rules! impl_into_closure {
    (
        $($ty:ident),*
    ) => {
        impl<S, T, F, $($ty,)*> IntoClosure<($($ty,)*), S> for T
        where
            T: Fn($($ty,)*) -> F,
            F: Future<Output = ()> + 'static,
            $( $ty: FromRequestParts<S>, )*
        {
        }
    };
}

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!(T1);
        $name!(T1, T2);
        $name!(T1, T2, T3);
        $name!(T1, T2, T3, T4);
        $name!(T1, T2, T3, T4, T5);
        $name!(T1, T2, T3, T4, T5, T6);
        $name!(T1, T2, T3, T4, T5, T6, T7);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
    };
}

all_the_tuples!(impl_closure_trait);
all_the_tuples!(impl_into_closure);

#[cfg(test)]
mod tests {
    use axum::http::{request::Parts, Request};

    use crate::context::Context;

    fn parts() -> Parts {
        let req = Request::new(());
        let (parts, _) = req.into_parts();
        parts
    }

    #[tokio::test]
    async fn test_update_u32_state_in_closure() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state(0u32);

        let closure = ctx.use_closure(move || async move {
            state.set(1);
        });

        // we run the closure manually, not by calling call
        // call relies on the websocket loop to be running
        ctx.closures.run(closure.id, &parts(), &());
        ctx.closures.join_set.join_next().await.unwrap().unwrap();

        assert_eq!(1, state.get());
    }

    #[tokio::test]
    async fn test_update_string_state_in_closure() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state("my string".to_string());

        let closure = ctx.use_closure(move || async move {
            state.set("other string".to_string());
        });

        // we run the closure manually, not by calling call
        // call relies on the websocket loop to be running
        ctx.closures.run(closure.id, &parts(), &());
        ctx.closures.join_set.join_next().await.unwrap().unwrap();

        assert_eq!("other string", state.get());
    }
}

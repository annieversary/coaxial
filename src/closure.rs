use axum::{extract::FromRequestParts, http::request::Parts};
use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;

pub(crate) type Closures<S> = HashMap<String, Arc<dyn ClosureTrait<S>>>;

#[derive(Clone)]
pub struct Closure {
    pub(crate) id: String,
    pub(crate) closure_call_tx: UnboundedSender<Self>,
}

impl Closure {
    /// Queues the function to be run
    ///
    /// Note: this doesn't call the closure immediately.
    /// Keep in mind, the closure will not be run until the websocket connection has been established.
    pub fn call(&self) {
        self.closure_call_tx.send(self.clone()).unwrap();
    }
}

/// Trait used to type-erase all closures, so they can be stored in the same HashMap
pub trait ClosureTrait<S>: Send + Sync {
    fn call<'a>(&'a self, parts: Parts, state: S) -> Pin<Box<dyn Future<Output = ()> + 'a>>;
}

impl<S, F, Fut> ClosureTrait<S> for ClosureWrapper<F, ()>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = ()> + 'static,
{
    fn call(&self, _parts: Parts, _state: S) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
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
            Fut: Future<Output = ()> + 'static,
        $( $ty: FromRequestParts<S> + Send + Sync, )*
            S: 'static
        {
            fn call<'a>(
                &'a self,
                mut parts: Parts,
                state: S,
            ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
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

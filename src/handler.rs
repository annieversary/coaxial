use axum::extract::{FromRequestParts, Request};
use std::{future::Future, pin::Pin};

use crate::{context::Context, CoaxialResponse};

// TODO implement handler for everything else
pub trait CoaxialHandler<T, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = CoaxialResponse> + Send + 'static;
    fn call(self, req: Request, state: S) -> Self::Future;
}

// implement handler for the basic func that takes only the context
impl<F, Fut, S> CoaxialHandler<((),), S> for F
where
    F: FnOnce(Context) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = CoaxialResponse> + Send,
    // TODO we can add an IntoCoaxialResponse here
{
    type Future = Pin<Box<dyn Future<Output = CoaxialResponse> + Send>>;

    fn call(self, _req: Request, _state: S) -> Self::Future {
        Box::pin(async move { self(Context::default()).await })
    }
}
impl<F, Fut, S, T> CoaxialHandler<((T,),), S> for F
where
    F: FnOnce(Context, T) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = CoaxialResponse> + Send,
    S: Send + Sync + 'static,
    T: FromRequestParts<S>, // TODO we can add an IntoCoaxialResponse here
{
    type Future = Pin<Box<dyn Future<Output = CoaxialResponse> + Send>>;

    fn call(self, req: Request, state: S) -> Self::Future {
        Box::pin(async move {
            let (mut parts, _body) = req.into_parts();
            let state = &state;

            let t = match T::from_request_parts(&mut parts, state).await {
                Ok(value) => value,
                Err(_rejection) => panic!("rejection"),
            };

            self(Context::default(), t).await
        })
    }
}

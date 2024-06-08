use axum::extract::{FromRequest, FromRequestParts, Request};
use std::{future::Future, pin::Pin};

use crate::{context::Context, CoaxialResponse};

pub trait CoaxialHandler<T, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = CoaxialResponse<S>> + Send + 'static;
    fn call(self, req: Request, state: S) -> Self::Future;
}

// implement handler for the basic func that takes only the context
impl<F, Fut, S> CoaxialHandler<((),), S> for F
where
    F: FnOnce(Context<S>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = CoaxialResponse<S>> + Send,
    S: Send + Sync + 'static,
{
    type Future = Pin<Box<dyn Future<Output = CoaxialResponse<S>> + Send>>;

    fn call(self, _req: Request, _state: S) -> Self::Future {
        Box::pin(async move { self(Context::default()).await })
    }
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, S, M, $($ty,)* $last> CoaxialHandler<((M, $($ty,)* $last,),), S> for F
        where
            F: FnOnce(Context<S>, $($ty,)* $last,) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = CoaxialResponse<S>> + Send,
            S: Send + Sync + 'static,
            $( $ty: FromRequestParts<S> + Send, )*
            $last: FromRequest<S, M> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = CoaxialResponse<S>> + Send>>;

            fn call(self, req: Request, state: S) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, body) = req.into_parts();
                    let state = &state;

                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, state).await {
                            Ok(value) => value,
                            Err(_rejection) => todo!("rejections aren't handled yet"),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    let $last = match $last::from_request(req, state).await {
                        Ok(value) => value,
                        Err(_rejection) => todo!("rejections aren't handled yet"),
                    };

                    self(Context::default(), $($ty,)* $last,).await
                })
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([], T1);
        $name!([T1], T2);
        $name!([T1, T2], T3);
        $name!([T1, T2, T3], T4);
        $name!([T1, T2, T3, T4], T5);
        $name!([T1, T2, T3, T4, T5], T6);
        $name!([T1, T2, T3, T4, T5, T6], T7);
        $name!([T1, T2, T3, T4, T5, T6, T7], T8);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13], T14);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14], T15);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15], T16);
    };
}

all_the_tuples!(impl_handler);

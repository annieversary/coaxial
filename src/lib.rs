#[macro_use]
extern crate serde;

use axum::response::Response;

use context::Context;
use html::{AttributeValue, Component, Element};

mod closures;
pub mod computed;
pub mod config;
pub mod context;
mod events;
mod handler;
mod helpers;
pub mod html;
pub mod live;
mod random_id;
mod reactive_js;
mod states;
pub use states::StateGet;

pub type CoaxialResponse<S = ()> = Response<Output<S>>;
pub struct Output<S = ()> {
    element: Element,
    context: Context<S>,
}

pub mod family {
    use std::ops::{Deref, DerefMut};

    use crate::states::State;

    // https://rustyyato.github.io/type/system,type/families/2021/02/15/Type-Families-1.html
    // https://github.com/RustyYato/type-families/blob/main/src/gat.rs
    pub trait Family: Copy {
        type This<A: 'static>;

        fn from<A: 'static>(a: A) -> Self::This<A>;
    }

    // State is the one used on render - should probably be renamed to render
    // state contains the id and stuff
    #[derive(Clone, Copy)]
    pub struct StateFamily;
    impl Family for StateFamily {
        type This<A: 'static> = State<A>;

        fn from<A: 'static>(a: A) -> Self::This<A> {
            State {
                inner: todo!(),
                id: todo!(),
            }
        }
    }

    // then we have another family - called Mut - that implements DerefMut that does state tracking
    // this one allows us to tell what states have been set

    #[derive(Clone, Copy)]
    pub struct MutFamily;
    impl Family for MutFamily {
        // TODO this should be MUT and not option
        type This<A: 'static> = Mut<A>;

        fn from<A: 'static>(a: A) -> Self::This<A> {
            Mut(a)
        }
    }

    // TODO this goes somewhere else
    pub struct Mut<A>(A);
    impl<A> Deref for Mut<A> {
        type Target = A;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<A> DerefMut for Mut<A> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
}

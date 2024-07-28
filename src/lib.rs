#[macro_use]
extern crate serde;

use axum::response::Response;

use context::Context;
use html::Element;
use rand::Rng;

mod closure;
pub mod config;
pub mod context;
mod event_handlers;
mod handler;
mod helpers;
pub mod html;
pub mod live;
mod reactive_js;
mod state;

pub type CoaxialResponse<S = ()> = Response<Output<S>>;
pub struct Output<S = ()> {
    element: Element,
    context: Context<S>,
}

pub(crate) fn random_id<RNG: Rng>(rng: &mut RNG) -> String {
    use rand::distributions::Alphanumeric;

    rng.sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

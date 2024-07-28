#[macro_use]
extern crate serde;

use axum::response::Response;

use context::Context;
use html::Element;

mod closure;
pub mod config;
pub mod context;
mod event_handlers;
mod handler;
mod helpers;
pub mod html;
pub mod live;
mod random_id;
mod reactive_js;
mod state;

pub type CoaxialResponse<S = ()> = Response<Output<S>>;
pub struct Output<S = ()> {
    element: Element,
    context: Context<S>,
}

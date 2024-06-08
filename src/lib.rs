#[macro_use]
extern crate serde;

use std::sync::Arc;

use axum::{response::Response, Extension};

use context::Context;
use html::Element;

mod closure;
pub mod context;
mod event_handlers;
mod handler;
mod helpers;
pub mod html;
pub mod live;
mod state;

/// Configuration for Coaxial.
///
/// Should be added as a layer for the routes.
#[derive(Clone)]
pub struct Config {
    layout: Arc<dyn Layout + Send + Sync + 'static>,
}

impl Config {
    pub fn with_layout<F>(layout: F) -> Self
    where
        F: Fn(Element) -> Element + Send + Sync + 'static,
    {
        let layout = move |body| {
            let mut out = layout(body);
            out.content.push_str(coaxial_adapter());
            out
        };

        Config {
            layout: Arc::new(layout),
        }
    }

    pub fn layer(self) -> Extension<Self> {
        Extension(self)
    }
}

impl Default for Config {
    fn default() -> Self {
        use html::{body, head, html};
        Config::with_layout(|content| html(head(()) + body(content)))
    }
}

pub type CoaxialResponse = Response<Output>;
pub struct Output {
    element: Element,
    context: Context,
}

/// Returns a string containing an HTML `<script>` tag containing the adapter JS code.
pub fn coaxial_adapter() -> &'static str {
    include_str!("base.html")
}

pub trait Layout {
    fn call(&self, content: Element) -> Element;
}
impl<F> Layout for F
where
    F: Fn(Element) -> Element,
{
    fn call(&self, content: Element) -> Element {
        (self)(content)
    }
}

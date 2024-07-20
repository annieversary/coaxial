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
        use html::{body, head, html, Content};

        Config::with_layout(|content| {
            html(
                Content::Children(vec![
                    head(Content::Empty, Default::default()),
                    body(
                        Content::Children(vec![content, coaxial_adapter_script()]),
                        Default::default(),
                    ),
                ]),
                Default::default(),
            )
        })
    }
}

pub type CoaxialResponse<S = ()> = Response<Output<S>>;
pub struct Output<S = ()> {
    element: Element,
    context: Context<S>,
}

/// Returns a string containing an HTML `<script>` tag containing the adapter JS code.
pub fn coaxial_adapter_script() -> Element {
    Element {
        name: "script".to_string(),
        content: html::Content::Raw(include_str!("base.js").to_string()),
        attributes: Default::default(),
    }
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

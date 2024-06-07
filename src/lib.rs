use axum::{response::Response, Extension};

use context::Context;
use html::Element;

mod closure;
pub mod context;
mod handler;
pub mod html;
pub mod live;
mod state;

/// Configuration for Coaxial.
///
/// Should be added as a layer for the routes.
#[derive(Clone)]
pub struct Config {
    layout: String,
}

impl Config {
    pub fn with_layout(layout: Element) -> Self {
        let mut layout = layout.content;
        layout.push_str(coaxial_adapter());
        Config { layout }
    }

    pub fn layer(self) -> Extension<Self> {
        Extension(self)
    }
}

impl Default for Config {
    fn default() -> Self {
        use html::{body, head, html, slot};
        Config::with_layout(html(head(()) + body(slot())))
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

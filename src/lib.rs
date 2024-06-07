use axum::{response::Response, Extension};

use context::Context;
use html::Element;

mod closure;
pub mod context;
mod handler;
pub mod html;
pub mod live;
mod state;

#[derive(Clone)]
pub struct Coaxial {
    layout: String,
}

impl Coaxial {
    pub fn with_layout(layout: Element) -> Extension<Self> {
        let mut layout = layout.content;
        layout.push_str(include_str!("base.html"));
        Extension(Coaxial { layout })
    }
}

pub type CoaxialResponse = Response<Output>;
pub struct Output {
    element: Element,
    context: Context,
}

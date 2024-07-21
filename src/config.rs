use std::sync::Arc;

use axum::Extension;

use crate::html::{Content, Element};

/// Configuration for Coaxial.
///
/// Should be added as a layer for the routes.
#[derive(Clone)]
pub struct Config {
    pub(crate) layout: Arc<dyn Layout + Send + Sync + 'static>,
}

impl Config {
    pub fn with_layout<F>(layout: F) -> Self
    where
        F: Fn(Element, Element) -> Element + Send + Sync + 'static,
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
        use crate::html::{body, head, html};

        Config::with_layout(|content, coaxial_adapter| {
            html(
                Content::List(vec![
                    head(Content::Empty, Default::default()).into(),
                    body(
                        Content::List(vec![content.into(), coaxial_adapter.into()]),
                        Default::default(),
                    )
                    .into(),
                ]),
                Default::default(),
            )
        })
    }
}

pub trait Layout {
    fn call(&self, content: Element, scripts: Element) -> Element;
}
impl<F> Layout for F
where
    F: Fn(Element, Element) -> Element,
{
    fn call(&self, content: Element, scripts: Element) -> Element {
        (self)(content, scripts)
    }
}

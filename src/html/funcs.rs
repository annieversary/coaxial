use super::{Attributes, Content, Element};

macro_rules! make_elements_funcs {
    ($($name:ident),* $(,)?) => {
        $(
            pub fn $name(content: impl Into<Content>, attributes: Attributes) -> Element {
                Element {
                    id: None,
                    name: stringify!($name).to_string(),
                    content: content.into(),
                    attributes,
                }
            }
        )*
    };
}

make_elements_funcs!(
    div, html, head, body, p, a, button, section, aside, main, script, strong, b, i, em, style
);

macro_rules! make_void_elements {
    ($($name:ident),* $(,)?) => {
        /// HTML elements that cannot have any child nodes
        ///
        /// https://developer.mozilla.org/en-US/docs/Glossary/Void_element
        pub(crate) const VOID_ELEMENTS: &[&str] = &[ $(stringify!($name)),* ];

        $(
            pub fn $name(attributes: Attributes) -> Element {
                Element {
                    id: None,
                    name: stringify!($name).to_string(),
                    content: Content::Empty,
                    attributes,
                }
            }
        )*
    };
}

make_void_elements!(
    area, base, br, col, embed, hr, img, input, link, meta, param, source, track, wbr,
);

pub(crate) const DOCTYPE_HTML: &str = "<!DOCTYPE html>";

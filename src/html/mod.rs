#[macro_export]
macro_rules! attrs {
    ( $( $attr:expr => $value:expr ),* $(,)?) => {
        {
            let mut attributes = $crate::html::Attributes::default();

            $(
                attributes.insert($attr, $value);
            )*

            attributes
        }
    };
}

mod attribute;
mod attributes;
mod content;
mod element;
mod funcs;

pub use attribute::{Attribute, AttributeValue, ClosureDescriptor, StateDescriptor};
pub use attributes::Attributes;
pub use content::{Content, ContentValue};
pub use element::Element;
pub use funcs::*;

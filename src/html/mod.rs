#[macro_export]
macro_rules! attrs {
    ( $( $attr:expr => $value:expr ),* $(,)?) => {
        $crate::html::Attributes::new(
            vec![$( ($attr.to_string(), $crate::html::Attribute::from($value)), )*]
        )
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

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

use crate::family::{Family, MutFamily, StateFamily};

pub trait Component: Sized {
    type Component<FAM: Family>;

    type Message: Message<Self>;

    fn make_generic<FAM: Family>(self) -> Self::Component<FAM>;
}

pub trait Renderable: Component {
    fn render(component: &State<Self>) -> Element;
}

pub type State<C> = <C as Component>::Component<StateFamily>;

pub type Mut<C> = <C as Component>::Component<MutFamily>;

pub trait Message<C: Component> {
    // TODO can this have a return type? idk what that would do
    fn handle(&self, component: &mut Mut<C>);
}

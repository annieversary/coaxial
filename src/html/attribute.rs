use std::fmt::Display;

use crate::{
    closure::Closure,
    random_id::RandomId,
    reactive_js::{Content, ElementAttributeReactivityDescriptor, Reactivity},
    state::State,
};

#[derive(Default, Debug, PartialEq, Eq)]
pub enum Attribute {
    #[default]
    Empty,
    Raw(String),
    Text(String),
    State(StateDescriptor),
    Closure(ClosureDescriptor),
    // TODO we probably do want the ability to do lists here
}

impl Attribute {
    pub(crate) fn is_reactive(&self) -> bool {
        match self {
            Attribute::Empty => false,
            Attribute::Raw(_) => false,
            Attribute::Text(_) => false,
            Attribute::Closure(_) => false,

            Attribute::State(_) => true,
        }
    }

    pub(crate) fn render(&self, output: &mut String) {
        match self {
            Attribute::Raw(text) => output.push_str(text),
            Attribute::Text(text) => {
                output.push_str(&html_escape::encode_double_quoted_attribute(text))
            }
            // TODO this needs to include something that updates it
            // probably outside of it, as generated code
            Attribute::State(desc) => {
                output.push_str(&desc.display);

                // push_strs!(output =>
                //     &desc.display, "\" coax-change-", &desc.state_id, "=\"", key,
                // );

                // if key == "value" || key == "checked" {
                //     push_strs!(output =>
                //         "\" onchange=\"window.Coaxial.setValue(",
                //         &desc.state_id, ", ['", key, "'])"
                //     );
                // }
            }
            Attribute::Closure(desc) => {
                output.push_str("window.Coaxial.callClosure('");
                desc.closure_id.fmt(output).unwrap();
                output.push_str("')");
            }
            Attribute::Empty => {}
        }
    }

    pub(crate) fn reactivity<'a, 'b>(
        &'a self,
        element_id: Option<RandomId>,
        key: &'a str,
        reactivity: &'b mut Reactivity<'a>,
    ) where
        'a: 'b,
    {
        // so im not sure how we want to deal with closures
        // cause we can do the onclick="window.Coaxial.callClosure()" thing,
        // but that will:
        // 1) not work for Lists (not sure why a closure would be in a list)
        // 2) not work if the attribute is something that isn't run as JS
        // im thinking that someone could do like a (data-function => closure), and then try to run said closure from their own js

        match self {
            Attribute::Empty => {}
            Attribute::Raw(_) => {}
            Attribute::Text(_) => {}
            Attribute::State(state_descriptor) => {
                let Some(element_id) = element_id else { return };

                reactivity.add_element_attribute(ElementAttributeReactivityDescriptor {
                    element_id,
                    attribute_key: key,

                    state_descriptors: vec![state_descriptor],
                    content: vec![Content::Var(0)],
                });
            }
            Attribute::Closure(_) => {
                // TODO i dont know if we want to do something here, or if we should stay with rendering the
                // `window.Coaxial.callClosure(...)`
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct StateDescriptor {
    pub(crate) display: String,
    pub(crate) state_id: String,
}
impl<T> From<State<T>> for StateDescriptor
where
    T: Clone + Display + Send + Sync + 'static,
{
    fn from(value: State<T>) -> Self {
        Self {
            display: value.get().to_string(),
            state_id: value.id.to_string(),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct ClosureDescriptor {
    pub(crate) closure_id: RandomId,
}
impl From<Closure> for ClosureDescriptor {
    fn from(value: Closure) -> Self {
        Self {
            closure_id: value.id,
        }
    }
}

impl From<()> for Attribute {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}
impl From<String> for Attribute {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl<'a> From<&'a str> for Attribute {
    fn from(value: &'a str) -> Self {
        Self::Text(value.to_string())
    }
}
impl From<Closure> for Attribute {
    fn from(value: Closure) -> Self {
        Self::Closure(value.into())
    }
}
impl<T> From<State<T>> for Attribute
where
    T: Clone + Display + Send + Sync + 'static,
{
    fn from(value: State<T>) -> Self {
        Self::State(value.into())
    }
}

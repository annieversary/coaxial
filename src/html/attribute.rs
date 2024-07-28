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
    Value(AttributeValue),
    List(Vec<AttributeValue>),
}

impl Attribute {
    pub(crate) fn is_reactive(&self) -> bool {
        match self {
            Attribute::Empty => false,
            Attribute::Value(value) => value.is_reactive(),
            Attribute::List(list) => list.iter().any(AttributeValue::is_reactive),
        }
    }

    pub(crate) fn render(&self, output: &mut String) {
        match self {
            Attribute::Empty => {}
            Attribute::Value(value) => value.render(output),
            Attribute::List(list) => {
                for item in list {
                    item.render(output);
                }
            }
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
        match self {
            Self::Value(AttributeValue::State(state_descriptor)) => {
                let Some(element_id) = element_id else { return };

                reactivity.add_element_attribute(ElementAttributeReactivityDescriptor {
                    element_id,
                    attribute_key: key,

                    state_descriptors: vec![state_descriptor],
                    content: vec![Content::Var(0)],
                });
            }
            Attribute::List(list) => {
                let Some(element_id) = element_id else { return };

                let state_descriptors = list.iter().filter_map(|c| c.state()).collect::<Vec<_>>();

                let content = list
                    .iter()
                    .map(|value| match value {
                        AttributeValue::Raw(text) => Content::Text(text.into()),
                        AttributeValue::Text(text) => {
                            Content::Text(html_escape::encode_script_single_quoted_text(text))
                        }
                        AttributeValue::State(descriptor) => Content::Var(
                            state_descriptors
                                .iter()
                                .position(|s| *s == descriptor)
                                .expect(
                                "state_descriptors always includes all the states that appear in the group",
                            ),
                        ),
                        AttributeValue::Closure(_) => todo!(),
                    })
                    .collect();

                reactivity.add_element_attribute(ElementAttributeReactivityDescriptor {
                    element_id,
                    attribute_key: key,

                    state_descriptors,
                    content,
                });
            }

            Self::Empty => {}
            Self::Value(_) => {}
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AttributeValue {
    Raw(String),
    Text(String),
    State(StateDescriptor),
    Closure(ClosureDescriptor),
}

impl AttributeValue {
    fn state(&self) -> Option<&StateDescriptor> {
        if let Self::State(state) = self {
            Some(state)
        } else {
            None
        }
    }

    pub(crate) fn is_reactive(&self) -> bool {
        match self {
            Self::Raw(_) => false,
            Self::Text(_) => false,
            Self::Closure(_) => false,

            Self::State(_) => true,
        }
    }

    pub(crate) fn render(&self, output: &mut String) {
        match self {
            Self::Raw(text) => output.push_str(text),
            Self::Text(text) => output.push_str(&html_escape::encode_double_quoted_attribute(text)),
            // TODO this needs to include something that updates it
            // probably outside of it, as generated code
            Self::State(desc) => {
                output.push_str(&desc.display);

                // push_strs!(output =>
                //     &desc.display, "\" coax-change-", &desc.state_id, "=\"", key,
                // );

                // if key == "value" || key == "checked" {
                //     push_strs!(output =>
                //         "\" onchange=\"window.Coaxial.setState(",
                //         &desc.state_id, ", ['", key, "'])"
                //     );
                // }
            }
            Self::Closure(desc) => {
                // TODO im pretty sure this is not it now

                // so im not sure how we want to deal with closures
                // cause we can do the onclick="window.Coaxial.callClosure()" thing,
                // but that will:
                // 1) not work for Lists (not sure why a closure would be in a list)
                // 2) not work if the attribute is something that isn't run as JS
                // im thinking that someone could do like a (data-function => closure), and then try to run said closure from their own js

                output.push_str("window.Coaxial.callClosure('");
                desc.closure_id.fmt(output).unwrap();
                output.push_str("')");
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

impl From<String> for AttributeValue {
    fn from(value: String) -> Self {
        AttributeValue::Text(value)
    }
}
impl<'a> From<&'a str> for AttributeValue {
    fn from(value: &'a str) -> Self {
        AttributeValue::Text(value.to_string())
    }
}
impl From<Closure> for AttributeValue {
    fn from(value: Closure) -> Self {
        AttributeValue::Closure(value.into())
    }
}
impl<T> From<State<T>> for AttributeValue
where
    T: Clone + Display + Send + Sync + 'static,
{
    fn from(value: State<T>) -> Self {
        AttributeValue::State(value.into())
    }
}

impl From<()> for Attribute {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}
impl<T> From<T> for Attribute
where
    AttributeValue: From<T>,
{
    fn from(value: T) -> Self {
        Self::Value(value.into())
    }
}

macro_rules! impl_into_attribute_tuple {
    (
        $($ty:ident),*
    ) => {
        #[allow(non_snake_case)]
        impl<$($ty,)*> From<($($ty,)*)> for Attribute
        where
            $( AttributeValue: From<$ty>, )*
        {
            fn from(($($ty,)*): ($($ty,)*)) -> Self {
                Self::List(vec![
                    $($ty.into(),)*
                ])
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!(T1);
        $name!(T1, T2);
        $name!(T1, T2, T3);
        $name!(T1, T2, T3, T4);
        $name!(T1, T2, T3, T4, T5);
        $name!(T1, T2, T3, T4, T5, T6);
        $name!(T1, T2, T3, T4, T5, T6, T7);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
    };
}

all_the_tuples!(impl_into_attribute_tuple);

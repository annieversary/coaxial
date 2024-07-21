use std::fmt::Display;

use crate::{closure::Closure, state::State};

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
    pub(crate) closure_id: String,
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

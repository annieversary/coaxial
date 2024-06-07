use std::{fmt::Display, ops::Add};

use crate::state::State;

#[derive(Default)]
pub struct Element {
    pub(crate) content: String,
}

#[derive(Default)]
pub struct ElementParams {
    children: Element,
    attributes: Attributes,
}

#[derive(Default)]
pub struct Attributes {
    list: Vec<(String, String)>,
}

impl Attributes {
    pub fn new(list: Vec<(String, String)>) -> Self {
        Self { list }
    }
}

impl std::fmt::Display for Attributes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (attribute, value) in &self.list {
            write!(f, " {attribute}=\"{value}\"")?;
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! attrs {
    ( $( ($attr:expr, $value:expr) ),* ) => {
        $crate::html::Attributes::new(
             vec![$( ($attr.to_string(), $value.to_string()), )*]
        )
    };
}

impl From<()> for Element {
    fn from(_val: ()) -> Self {
        Element {
            content: "".to_string(),
        }
    }
}
impl From<&'static str> for Element {
    fn from(val: &'static str) -> Self {
        Element {
            content: val.to_string(),
        }
    }
}
impl<T: Display + Clone + Send + Sync> From<State<T>> for Element {
    fn from(value: State<T>) -> Self {
        Element {
            content: format!(
                "<span data-coaxial-id=\"{}\">{}</span>",
                value.id,
                value.get()
            ),
        }
    }
}

impl From<()> for ElementParams {
    fn from(_val: ()) -> Self {
        ElementParams::default()
    }
}
impl From<&'static str> for ElementParams {
    fn from(children: &'static str) -> Self {
        ElementParams {
            children: children.into(),
            attributes: Attributes::default(),
        }
    }
}
impl From<Element> for ElementParams {
    fn from(children: Element) -> Self {
        ElementParams {
            children,
            attributes: Attributes::default(),
        }
    }
}
impl<T: Display + Clone + Send + Sync> From<State<T>> for ElementParams {
    fn from(state: State<T>) -> Self {
        ElementParams {
            children: state.into(),
            attributes: Attributes::default(),
        }
    }
}

impl<A: ToString, B: ToString> From<(A, B)> for Attributes {
    fn from((a, b): (A, B)) -> Self {
        Self {
            list: vec![(a.to_string(), b.to_string())],
        }
    }
}
impl From<Vec<(String, String)>> for Attributes {
    fn from(list: Vec<(String, String)>) -> Self {
        Self { list }
    }
}

impl<C: Into<Element>, A: Into<Attributes>> From<(C, A)> for ElementParams {
    fn from((children, attributes): (C, A)) -> Self {
        ElementParams {
            children: children.into(),
            attributes: attributes.into(),
        }
    }
}

impl Add<Self> for Element {
    type Output = Self;

    fn add(mut self, rhs: Element) -> Self::Output {
        self.content.push_str(&rhs.content);
        self
    }
}

macro_rules! make_element {
    ($ident:ident) => {
        pub fn $ident(params: impl Into<ElementParams>) -> Element {
            let ElementParams {
                mut children,
                attributes,
            } = params.into();

            let attributes = attributes.to_string();

            children.content = format!(
                "<{}{attributes}>{}</{}>",
                stringify!($ident),
                children.content,
                stringify!($ident)
            );

            children
        }
    };
}

make_element!(div);
make_element!(p);
make_element!(button);
make_element!(html);
make_element!(body);
make_element!(head);

pub fn slot() -> Element {
    Element {
        content: "{^slot^}".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let el = div(p("hello") + p("world"));

        assert_eq!(el.content, "<div><p>hello</p><p>world</p></div>");
    }

    #[test]
    fn test_attributes() {
        let el = div(("hello", vec![("hi".to_string(), "test".to_string())]));

        assert_eq!(el.content, "<div hi=\"test\">hello</div>");
    }

    #[test]
    fn test_attributes_macro() {
        let el = div(("hello", attrs![("hi", "test")]));

        assert_eq!(el.content, "<div hi=\"test\">hello</div>");
    }
}

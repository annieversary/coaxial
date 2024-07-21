use std::fmt::Display;

use rand::Rng;

use crate::{closure::Closure, random_id, state::State};

macro_rules! push_strs {
    ( $output:ident => $($vals:expr),* $(,)? ) => {
        $(
            $output.push_str($vals);
        )*
    };
}

#[derive(Debug, PartialEq, Eq)]
pub struct Element {
    pub(crate) id: Option<String>,
    pub(crate) name: String,
    pub(crate) content: Content,
    pub(crate) attributes: Attributes,
}

impl Element {
    pub(crate) fn optimize(&mut self) {
        self.content.optimize();
    }

    pub(crate) fn is_reactive(&self) -> bool {
        self.content.is_reactive() || self.attributes.is_reactive()
    }

    pub(crate) fn give_ids<RNG: Rng>(&mut self, rng: &mut RNG) {
        if self.is_reactive() && self.id.is_none() {
            self.id = Some(random_id(rng));
        }

        self.content.give_ids(rng);
    }

    pub(crate) fn render(&self, output: &mut String) {
        output.push('<');
        output.push_str(&self.name);

        if !self.attributes.list.is_empty() {
            output.push(' ');
            self.attributes.render(output);
        }

        // void elements cannot have a closing tag
        if VOID_ELEMENTS.contains(&self.name.as_str()) {
            output.push_str(" />");
            return;
        }

        if let Some(id) = &self.id {
            push_strs!(output => " coax-id=\"", id, "\"");
        }

        self.content.reactive_attributes(output);

        output.push('>');

        self.content.render(output);

        output.push_str("</");
        output.push_str(&self.name);
        output.push('>');
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub enum Content {
    #[default]
    Empty,
    Raw(String),
    Text(String),
    Element(Box<Element>),
    List(Vec<Content>),
    State(StateDescriptor),
}

impl Content {
    /// Turns this Content into it's canonical form
    ///
    /// For example, a `Content::List` with an empty list will be transformed into a `Content::Empty`.
    pub(crate) fn optimize(&mut self) {
        match self {
            Content::Empty => {}
            Content::Raw(_) => {}
            Content::Text(_) => {}
            Content::State(_) => {}
            Content::Element(element) => element.optimize(),
            Content::List(list) => {
                for item in list.iter_mut() {
                    item.optimize();
                }

                match list.len() {
                    0 => {
                        // if the list is empty, change it for an empty
                        *self = Content::Empty;
                    }
                    1 => {
                        // if there is a single element in the list, promote it
                        *self = list.remove(0);
                    }
                    _ => {
                        Self::optimize_list(list);
                        if list.is_empty() {
                            *self = Content::Empty;
                        } else if list.len() == 1 {
                            *self = list.remove(0);
                        }
                    }
                }
            }
        }
    }

    fn optimize_list(list: &mut Vec<Content>) {
        // remove empty elements from a list before we process it
        list.retain(|content| !matches!(content, Content::Empty));

        // adjacent text contents should be merged, etc
        let mut i = 0;
        while i + 1 < list.len() {
            if matches!(list[i], Content::Text(_) | Content::Raw(_))
                && matches!(list[i + 1], Content::Text(_) | Content::Raw(_))
            {
                let mut next = list.remove(i + 1);

                next.text_to_raw();
                list[i].text_to_raw();

                let Content::Raw(current) = &mut list[i] else {
                    unreachable!();
                };
                let Content::Raw(next) = next else {
                    unreachable!();
                };

                current.push_str(&next);
            }

            i += 1;
        }
    }

    fn text_to_raw(&mut self) {
        if let Content::Text(string) = self {
            *self = Content::Raw(html_escape::encode_text(string).to_string());
        }
    }

    pub(crate) fn give_ids<RNG: Rng>(&mut self, rng: &mut RNG) {
        match self {
            Content::List(list) => {
                for item in list {
                    item.give_ids(rng);
                }
            }
            Content::Element(element) => element.give_ids(rng),
            Content::State(_) => {}
            Content::Empty => {}
            Content::Raw(_) => {}
            Content::Text(_) => {}
        }
    }

    pub(crate) fn is_reactive(&self) -> bool {
        match self {
            Content::List(list) => list.iter().any(Self::is_reactive),
            Content::State(_) => true,

            Content::Empty => false,
            Content::Raw(_) => false,
            Content::Text(_) => false,
            Content::Element(_) => false,
        }
    }

    pub(crate) fn reactive_attributes(&self, output: &mut String) {
        // TODO so, ideally, this wouldn't actually add attributes
        // we'd generate some sort of js that modifies the code for this
        // so like in here we actually give this element a data-something with a random string
        // and generate the js that references that and contains the exact code to edit this
        match self {
            Content::List(_list) => {}
            Content::State(desc) => {
                push_strs!(output => " coax-change-", &desc.state_id, "=\"innerHTML\"",);
            }

            _ => {}
        }
    }

    pub(crate) fn render(&self, output: &mut String) {
        match self {
            Content::Empty => {}
            Content::Raw(raw) => output.push_str(raw),
            Content::Text(escaped) => output.push_str(&html_escape::encode_text(escaped)),
            Content::Element(child) => child.render(output),
            Content::List(list) => {
                for content in list {
                    content.render(output);
                }
            }
            Content::State(desc) => output.push_str(&desc.display),
        }
    }
}

impl From<()> for Content {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}
impl From<String> for Content {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl<'a> From<&'a str> for Content {
    fn from(value: &'a str) -> Self {
        Self::Text(value.to_string())
    }
}
impl From<Element> for Content {
    fn from(element: Element) -> Self {
        Self::Element(Box::new(element))
    }
}
impl From<Vec<Content>> for Content {
    fn from(value: Vec<Content>) -> Self {
        Self::List(value)
    }
}
impl<T> From<State<T>> for Content
where
    T: Clone + Display + Send + Sync + 'static,
{
    fn from(value: State<T>) -> Self {
        Self::State(value.into())
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Attributes {
    list: Vec<(String, Attribute)>,
}

impl Attributes {
    pub fn new(list: Vec<(String, Attribute)>) -> Self {
        Self { list }
    }

    pub(crate) fn is_reactive(&self) -> bool {
        self.list
            .iter()
            .any(|(_, attr)| Attribute::is_reactive(attr))
    }

    pub(crate) fn render(&self, output: &mut String) {
        for (key, attr) in &self.list {
            output.push_str(key);

            if matches!(attr, Attribute::Empty) {
                continue;
            }

            output.push_str("=\"");

            match attr {
                Attribute::Raw(text) => output.push_str(text),
                Attribute::Text(text) => {
                    output.push_str(&html_escape::encode_double_quoted_attribute(text))
                }
                // TODO this needs to include something that updates it
                // probably outside of it, as generated code
                Attribute::State(desc) => {
                    push_strs!(output =>
                        &desc.display, "\" coax-change-", &desc.state_id, "=\"", key,
                    );

                    if key == "value" || key == "checked" {
                        push_strs!(output =>
                            "\" onchange=\"window.Coaxial.setValue(",
                            &desc.state_id, ", ['", key, "'])"
                        );
                    }
                }
                Attribute::Closure(desc) => {
                    output.push_str("window.Coaxial.callClosure('");
                    output.push_str(&desc.closure_id);
                    output.push_str("')");
                }
                Attribute::Empty => unreachable!(),
            }

            output.push('"');
        }
    }
}

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
    display: String,
    state_id: String,
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
    closure_id: String,
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

#[macro_export]
macro_rules! attrs {
    ( $( $attr:expr => $value:expr ),* $(,)?) => {
        $crate::html::Attributes::new(
            vec![$( ($attr.to_string(), $crate::html::Attribute::from($value)), )*]
        )
    };
}

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

make_elements_funcs!(div, html, head, body, p, a, button, section, aside, main, script);

macro_rules! make_void_elements {
    ($($name:ident),* $(,)?) => {
        /// HTML elements that cannot have any child nodes
        ///
        /// https://developer.mozilla.org/en-US/docs/Glossary/Void_element
        const VOID_ELEMENTS: &[&str] = &[ $(stringify!($name)),* ];

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

#[cfg(test)]
mod tests {
    use rand::rngs::mock::StepRng;

    use super::*;

    #[test]
    fn test_basic() {
        let el = Element {
            id: Some("el1".to_string()),
            name: "div".to_string(),
            content: Content::List(vec![
                Element {
                    id: Some("el2".to_string()),
                    name: "p".to_string(),
                    content: Content::Text("hello".to_string()),
                    attributes: Default::default(),
                }
                .into(),
                Element {
                    id: None,
                    name: "p".to_string(),
                    content: Content::Text("world".to_string()),
                    attributes: Default::default(),
                }
                .into(),
            ]),
            attributes: Default::default(),
        };

        let mut output = String::new();
        el.render(&mut output);

        assert_eq!(
            output,
            "<div coax-id=\"el1\"><p coax-id=\"el2\">hello</p><p>world</p></div>"
        );
    }

    #[test]
    fn test_element_functions() {
        let el = div(
            Content::List(vec![p(
                Content::Text("hello".to_string()),
                Default::default(),
            )
            .into()]),
            Default::default(),
        );

        let mut output = String::new();
        el.render(&mut output);

        assert_eq!(output, "<div><p>hello</p></div>");
    }

    #[test]
    fn test_can_render_attributes() {
        let attrs = attrs!(
            "onclick" => Attribute::Text("hey".to_string()),
        );

        let mut output = String::new();
        attrs.render(&mut output);

        assert_eq!(output, "onclick=\"hey\"");
    }

    #[test]
    fn test_reactive_elements_have_ids() {
        let mut el = Element {
            id: None,
            name: "div".to_string(),
            content: Content::State(StateDescriptor {
                display: "value".to_string(),
                state_id: "my_state".to_string(),
            }),

            attributes: Default::default(),
        };

        el.give_ids(&mut StepRng::new(0, 1));

        assert!(el.content.is_reactive());
        assert!(el.id.is_some());
    }

    #[test]
    fn test_non_reactive_elements_dont_have_ids() {
        let mut el = Element {
            id: None,
            name: "div".to_string(),
            content: Content::Raw("value".to_string()),
            attributes: Default::default(),
        };

        el.give_ids(&mut StepRng::new(0, 1));

        assert!(!el.content.is_reactive());
        assert!(el.id.is_none());
    }

    #[test]
    fn test_build_content() {
        macro_rules! run {
            ($provided:expr, $expect:expr) => {
                let mut content = $provided;
                content.optimize();

                assert_eq!($expect, content);
            };
        }

        run!(Content::List(vec![]), Content::Empty);
        run!(
            Content::List(vec![Content::Empty, Content::Empty, Content::Empty]),
            Content::Empty
        );
        run!(
            Content::List(vec![Content::List(vec![Content::List(vec![])])]),
            Content::Empty
        );
        run!(
            Content::List(vec![Content::Raw("hey".to_string())]),
            Content::Raw("hey".to_string())
        );
        run!(
            Content::List(vec![
                Content::List(vec![Content::Raw("hey".to_string()),]),
                Content::Empty,
                Content::Text("hi".to_string())
            ]),
            Content::Raw("heyhi".to_string())
        );
    }
}

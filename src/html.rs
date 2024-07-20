use std::fmt::Display;

use crate::{closure::Closure, state::State};

macro_rules! push_strs {
    ( $output:ident => $($vals:expr),* $(,)? ) => {
        $(
            $output.push_str($vals);
        )*
    };
}

#[derive(Default)]
pub struct Element {
    pub(crate) name: String,
    pub(crate) content: Content,
    pub(crate) attributes: Attributes,
}

impl Element {
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

        self.content.reactive_attributes(output);

        output.push('>');

        self.content.render(output);

        output.push_str("</");
        output.push_str(&self.name);
        output.push('>');
    }
}

#[derive(Default)]
pub enum Content {
    #[default]
    Empty,
    Raw(String),
    Text(String),
    // TODO this technically means we can have a vec![Text, Children(vec![Element, Element])]
    // which doesn't make sense, since it'd really be a vec![Text, Element, Element]
    // TODO change to Element
    Children(Vec<Element>),
    List(Vec<Content>),
    State(StateDescriptor),
}
impl Content {
    pub(crate) fn reactive_attributes(&self, output: &mut String) {
        // TODO so, ideally, this wouldn't actually add attributes
        // we'd generate some sort of js that modifies the code for this
        match self {
            Content::List(_list) => {
                // TODO
                todo!("generate something that actually updates this as needed")
            }
            Content::State(desc) => {
                push_strs!(output => " coax-change-", &desc.state_id, "=\"innerHTML\"",);
            }

            // if this element has children, we don't consider it to be reactive, even if children are
            Content::Children(_) => {}
            _ => {}
        }
    }

    pub(crate) fn render(&self, output: &mut String) {
        match self {
            Content::Empty => {}
            Content::Raw(raw) => output.push_str(raw),
            Content::Text(escaped) => output.push_str(&html_escape::encode_text(escaped)),
            Content::Children(children) => {
                for child in children {
                    child.render(output);
                }
            }
            Content::List(list) => {
                // TODO if the list contains a state
                // add some code for updating this when state changes
                // since we will have to re-render it when the state updates
                for content in list {
                    content.render(output);
                }
            }
            // TODO add some code for updating this when this changed
            Content::State(desc) => output.push_str(&desc.display),
        }
    }
}

#[derive(Default)]
pub struct Attributes {
    list: Vec<(String, Attribute)>,
}

impl Attributes {
    pub fn new(list: Vec<(String, Attribute)>) -> Self {
        Self { list }
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

pub enum Attribute {
    Raw(String),
    Text(String),
    State(StateDescriptor),
    Closure(ClosureDescriptor),
    Empty,
}

pub struct StateDescriptor {
    display: String,
    state_id: String,
    // TODO unsure what other fields we actually will want here
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

#[macro_export]
macro_rules! attrs {
    ( $( $attr:expr => $value:expr ),* $(,)?) => {
        $crate::html::Attributes::new(
            // TODO this $value should have like. an Into or something
            vec![$( ($attr.to_string(), $value), )*]
        )
    };
}

macro_rules! make_elements_funcs {
    ($($name:ident),* $(,)?) => {
        $(
            pub fn $name(content: Content, attributes: Attributes) -> Element {
                Element {
                    name: stringify!($name).to_string(),
                    content,
                    attributes,
                }
            }
        )*
    };
}

make_elements_funcs!(div, html, head, body, p, a, button, section, aside, main,);

macro_rules! make_void_elements {
    ($($name:ident),* $(,)?) => {
        /// HTML elements that cannot have any child nodes
        ///
        /// https://developer.mozilla.org/en-US/docs/Glossary/Void_element
        const VOID_ELEMENTS: &[&str] = &[ $(stringify!($name)),* ];

        $(
            pub fn $name(attributes: Attributes) -> Element {
                Element {
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
    use super::*;

    #[test]
    fn test_basic() {
        let el = Element {
            name: "div".to_string(),
            content: Content::Children(vec![
                Element {
                    name: "p".to_string(),
                    content: Content::Text("hello".to_string()),
                    attributes: Default::default(),
                },
                Element {
                    name: "p".to_string(),
                    content: Content::Text("world".to_string()),
                    attributes: Default::default(),
                },
            ]),
            attributes: Default::default(),
        };

        let mut output = String::new();
        el.render(&mut output);

        assert_eq!(output, "<div><p>hello</p><p>world</p></div>");
    }

    #[test]
    fn test_element_functions() {
        let el = div(
            Content::Children(vec![p(
                Content::Text("hello".to_string()),
                Default::default(),
            )]),
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

    // #[test]
    // fn test_attributes() {
    //     let el = div(("hello", vec![("hi".to_string(), "test".to_string())]));

    //     assert_eq!(el.content, "<div hi=\"test\">hello</div>");
    // }

    // #[test]
    // fn test_attributes_macro() {
    //     let el = div(("hello", attrs![("hi", "test")]));

    //     assert_eq!(el.content, "<div hi=\"test\">hello</div>");
    // }
}

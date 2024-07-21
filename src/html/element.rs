use rand::Rng;

use crate::random_id;

use super::{Attributes, Content, VOID_ELEMENTS};

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

    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::mock::StepRng;

    use crate::html::{div, p, StateDescriptor};

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
}

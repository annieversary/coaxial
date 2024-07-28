use rand::Rng;

use crate::{random_id::RandomId, reactive_js::Reactivity};

use super::{Attributes, Content, VOID_ELEMENTS};

#[derive(Debug, PartialEq, Eq)]
pub struct Element {
    pub(crate) id: Option<RandomId>,
    pub(crate) name: String,
    pub(crate) content: Content,
    pub(crate) attributes: Attributes,
}

impl Element {
    pub(crate) fn optimize(&mut self) {
        self.content.optimize();
        // TODO self.attributes.optimize();
    }

    pub(crate) fn is_reactive(&self) -> bool {
        self.content.is_reactive() || self.attributes.is_reactive()
    }

    pub(crate) fn give_ids<RNG: Rng>(&mut self, rng: &mut RNG) {
        if self.is_reactive() && self.id.is_none() {
            self.id = Some(RandomId::from_rng(rng));
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
            output.push_str(" coax-id=\"");
            id.fmt(output).unwrap();
            output.push('\"');
        }

        output.push('>');

        self.content.render(output);

        output.push_str("</");
        output.push_str(&self.name);
        output.push('>');
    }

    pub(crate) fn reactivity<'a, 'b>(&'a self, reactivity: &'b mut Reactivity<'a>)
    where
        'a: 'b,
    {
        self.content.reactivity(self.id, reactivity);
        self.attributes.reactivity(self.id, reactivity);
    }

    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::mock::StepRng;

    use crate::html::{content::ContentValue, div, p, StateDescriptor};

    use super::*;

    #[test]
    fn test_basic() {
        let el = Element {
            id: Some(RandomId::from_str("aaaabbbb")),
            name: "div".to_string(),
            content: Content::List(vec![
                Element {
                    id: Some(RandomId::from_str("ccccdddd")),
                    name: "p".to_string(),
                    content: "hello".into(),
                    attributes: Default::default(),
                }
                .into(),
                Element {
                    id: None,
                    name: "p".to_string(),
                    content: "world".into(),
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
            "<div coax-id=\"aaaabbbb\"><p coax-id=\"ccccdddd\">hello</p><p>world</p></div>"
        );
    }

    #[test]
    fn test_element_functions() {
        let el = div(
            Content::List(vec![p("hello", Default::default()).into()]),
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
            content: Content::Value(ContentValue::State(StateDescriptor {
                display: "value".to_string(),
                state_id: "my_state".to_string(),
            })),

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
            content: "value".into(),
            attributes: Default::default(),
        };

        el.give_ids(&mut StepRng::new(0, 1));

        assert!(!el.content.is_reactive());
        assert!(el.id.is_none());
    }
}

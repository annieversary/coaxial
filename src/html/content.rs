use std::fmt::Display;

use crate::state::State;

use super::{attribute::StateDescriptor, element::Element};
use rand::Rng;

macro_rules! push_strs {
    ( $output:ident => $($vals:expr),* $(,)? ) => {
        $(
            $output.push_str($vals);
        )*
    };
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

#[cfg(test)]
mod tests {
    use super::*;

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

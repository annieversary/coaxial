use std::fmt::Display;

use crate::{
    random_id::RandomId,
    reactive_js::{Content as ReactiveContent, ElementContentReactivityDescriptor, Reactivity},
    state::State,
};

use super::{attribute::StateDescriptor, element::Element};
use rand::Rng;

#[derive(Default, Debug, PartialEq, Eq)]
pub enum Content {
    #[default]
    Empty,
    Raw(String),
    Text(String),
    Element(Box<Element>),
    State(StateDescriptor),
    List(Vec<Content>),
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
        // TODO this function might be more efficient if we roll all steps into one loop

        // a list in a list should be flattened
        let mut i = 0;
        while i + 1 < list.len() {
            if matches!(list[i], Content::List(_)) {
                let mut inner = Content::Empty;
                std::mem::swap(&mut inner, &mut list[i]);

                let Content::List(inner) = inner else {
                    unreachable!()
                };

                list.splice(i..i, inner);
            } else {
                // if we just spliced a list in, we want to keep going from the same index
                // otherwise, we advance the index and keep going
                i += 1;
            }
        }

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

    // TODO this function needs an exorcism
    pub(crate) fn reactivity<'a, 'b>(
        &'a self,
        element_id: Option<RandomId>,
        reactivity: &'b mut Reactivity<'a>,
    ) where
        'a: 'b,
    {
        match &self {
            Content::List(list) => {
                // so basically we wanna split it into groups of text/state and elements
                // then, elements can be taken care by recursion
                // and text/state groups can get a script that deals with this shit
                // and its smth like onStateChange(desc.state_id, v => doc.querySelector(...).childNodes[position in list of groups].textContent = v)

                let is_text = |content: &Content| -> bool {
                    matches!(
                        content,
                        Content::Raw(_) | Content::Text(_) | Content::State(_)
                    )
                };

                /// Add a group as a ReactiveDescriptor
                fn add_group<'a, 'b>(
                    reactivity: &'b mut Reactivity<'a>,
                    group: Vec<&'a Content>,
                    group_id: u32,
                    element_id: Option<RandomId>,
                ) where
                    'a: 'b,
                {
                    let Some(id) = element_id else { return };

                    let states = group.iter().filter_map(|c| c.state()).collect::<Vec<_>>();
                    reactivity.add_element_content(ElementContentReactivityDescriptor {
                        element_id: id,
                        child_node_idx: Some(group_id ),
                        content: group
                            .iter()
                            .map(|content| match content {
                                Content::Raw(text) => ReactiveContent::Text(text.into()),
                                Content::Text(text) => ReactiveContent::Text(
                                    html_escape::encode_script_single_quoted_text(text),
                                ),
                                Content::State(descriptor) => ReactiveContent::Var(
                                    states.iter().position(|s| *s == descriptor).expect("states always includes all the states that appear in the group"),
                                ),
                                _ => unreachable!("group only contains Raw, Text, and State"),
                            })
                            .collect(),
                        state_descriptors: states,
                    });
                }

                let mut group: Option<Vec<&Content>> = None;
                let mut group_id: u32 = 0;
                for content in list {
                    if is_text(content) {
                        if let Some(text) = &mut group {
                            text.push(content);
                        } else {
                            group = Some(vec![content]);
                            group_id += 1;
                        }
                    } else {
                        if let Some(group) = group.take() {
                            add_group(reactivity, group, group_id - 1, element_id);
                        }
                        group_id += 1;

                        if let Content::Element(element) = content {
                            element.reactivity(reactivity);
                        }
                        // we can ignore everything else
                        // we could handle nested lists, but we can assume they've already been flattened
                    }
                }

                // last thing might be a text group, so we need to deal with that
                if let Some(group) = group.take() {
                    add_group(reactivity, group, group_id - 1, element_id);
                }
            }
            Content::State(desc) => {
                let Some(id) = element_id else { return };

                reactivity.add_element_content(ElementContentReactivityDescriptor {
                    element_id: id,
                    child_node_idx: None,
                    state_descriptors: vec![desc],
                    content: vec![ReactiveContent::Var(0)],
                });
            }
            Content::Element(element) => element.reactivity(reactivity),

            _ => {}
        }
    }

    fn state(&self) -> Option<&StateDescriptor> {
        if let Content::State(state) = self {
            Some(state)
        } else {
            None
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
        // list in list should be flattened
        run!(
            Content::List(vec![
                Content::List(vec![
                    Content::State(StateDescriptor {
                        display: Default::default(),
                        state_id: Default::default()
                    }),
                    Content::Text("hey".to_string())
                ]),
                Content::State(StateDescriptor {
                    display: Default::default(),
                    state_id: Default::default()
                })
            ]),
            Content::List(vec![
                Content::State(StateDescriptor {
                    display: Default::default(),
                    state_id: Default::default()
                }),
                Content::Text("hey".to_string()),
                Content::State(StateDescriptor {
                    display: Default::default(),
                    state_id: Default::default()
                }),
            ])
        );
    }
}

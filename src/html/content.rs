use std::fmt::{Display, Write};

use crate::state::State;

use super::{attribute::StateDescriptor, element::Element};
use rand::Rng;

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

        // TODO a list in a list should be flattened

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
    pub(crate) fn reactive_scripts(&self, output: &mut String, element_id: Option<&str>) {
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

                let groups = {
                    let mut groups: Vec<(Vec<&Content>, u32)> = vec![];
                    let mut group: Option<Vec<&Content>> = None;
                    let mut group_id: u32 = 0;
                    for content in list {
                        if is_text(content) {
                            if let Some(text) = &mut group {
                                text.push(content);
                            } else {
                                group = Some(vec![content]);
                            }
                        } else {
                            if let Some(group) = group.take() {
                                groups.push((group, group_id));
                            }
                            group_id += 1;

                            if let Content::Element(element) = content {
                                element.reactive_scripts(output);
                            }
                            // we can ignore everything else
                            // we could handle nested lists, but we can assume they've already been flattened
                        }
                    }

                    groups
                };

                let Some(id) = element_id else { return };

                // so, each group can hold multiple states
                // we need to update them at the same time
                for (group, group_id) in groups {
                    output.push_str("window.Coaxial.onStateChange(['");

                    let mut state_count = 0;
                    for state_desc in group.iter().filter_map(|c| c.state()) {
                        output.push_str(&state_desc.state_id);
                        output.push_str("',");
                        state_count += 1;
                    }
                    output.push_str("], (");

                    for i in 0..state_count {
                        output.push('v');
                        output.push_str(&i.to_string());
                        output.push(',');
                    }

                    output
                        .write_fmt(format_args!(
                            ") => {{ if (el = document.querySelector('[coax-id=\"{}\"]')) if (child = el.childNodes[{}]) child.textContent = [",
                            id,
                            group_id,
                        ))
                        .unwrap();

                    state_count = 0;
                    for item in group {
                        match item {
                            Content::Raw(text) => {
                                output.push('\'');
                                output.push_str(text);
                                output.push('\'');
                            }
                            Content::Text(text) => {
                                output.push('\'');
                                output
                                    .push_str(&html_escape::encode_script_single_quoted_text(text));
                                output.push('\'');
                            }
                            Content::State(_) => {
                                output.push('v');
                                output.push_str(&state_count.to_string());

                                state_count += 1;
                            }
                            _ => unreachable!(),
                        }

                        output.push(',');
                    }

                    output.push_str("].join(''); });");
                }
            }
            Content::State(desc) => {
                let Some(id) = element_id else { return };

                output
                    .write_fmt(format_args!(
                        // TODO we should probably have a function on coaxial that does this already
                        "window.Coaxial.onStateChange('{}', v => {{ if (el = document.querySelector('[coax-id=\"{}\"]')) el.innerHTML = v.toString(); }});",
                        desc.state_id,
                        id,
                    ))
                    .unwrap();
            }
            Content::Element(element) => element.reactive_scripts(output),

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

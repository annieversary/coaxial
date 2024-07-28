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
    Value(ContentValue),
    List(Vec<ContentValue>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContentValue {
    Raw(String),
    Text(String),
    Element(Box<Element>),
    State(StateDescriptor),
}

impl ContentValue {
    fn text_to_raw(&mut self) {
        if let Self::Text(string) = self {
            *self = Self::Raw(html_escape::encode_text(string).to_string());
        }
    }

    pub(crate) fn is_reactive(&self) -> bool {
        match self {
            Self::State(_) => true,

            Self::Raw(_) => false,
            Self::Text(_) => false,
            Self::Element(_) => false,
        }
    }

    fn is_text(&self) -> bool {
        match self {
            Self::State(_) => true,
            Self::Raw(_) => true,
            Self::Text(_) => true,

            Self::Element(_) => false,
        }
    }

    fn state(&self) -> Option<&StateDescriptor> {
        if let ContentValue::State(state) = self {
            Some(state)
        } else {
            None
        }
    }

    pub(crate) fn render(&self, output: &mut String) {
        match self {
            Self::Raw(raw) => output.push_str(raw),
            Self::Text(escaped) => output.push_str(&html_escape::encode_text(escaped)),
            Self::Element(child) => child.render(output),
            Self::State(desc) => output.push_str(&desc.display),
        }
    }
}

impl Content {
    /// Turns this Content into it's canonical form
    ///
    /// For example, a `Content::List` with an empty list will be transformed into a `Content::Empty`.
    pub(crate) fn optimize(&mut self) {
        match self {
            Content::Value(ContentValue::Element(element)) => element.content.optimize(),
            Content::List(list) => {
                match list.len() {
                    0 => {
                        // if the list is empty, change it for an empty
                        *self = Content::Empty;
                    }
                    1 => {
                        // if there is a single element in the list, promote it
                        *self = Content::Value(list.remove(0));
                    }
                    _ => {
                        Self::optimize_list(list);
                        if list.is_empty() {
                            *self = Content::Empty;
                        } else if list.len() == 1 {
                            *self = Content::Value(list.remove(0));
                        }
                    }
                }
            }

            Content::Empty => {}
            Content::Value(ContentValue::Raw(_)) => {}
            Content::Value(ContentValue::Text(_)) => {}
            Content::Value(ContentValue::State(_)) => {}
        }
    }

    fn optimize_list(list: &mut Vec<ContentValue>) {
        // adjacent text contents should be merged, etc
        let mut i = 0;
        while i + 1 < list.len() {
            if matches!(list[i], ContentValue::Text(_) | ContentValue::Raw(_))
                && matches!(list[i + 1], ContentValue::Text(_) | ContentValue::Raw(_))
            {
                let mut next = list.remove(i + 1);

                next.text_to_raw();
                list[i].text_to_raw();

                let ContentValue::Raw(current) = &mut list[i] else {
                    unreachable!();
                };
                let ContentValue::Raw(next) = next else {
                    unreachable!();
                };

                current.push_str(&next);
            }

            i += 1;
        }
    }

    pub(crate) fn give_ids<RNG: Rng>(&mut self, rng: &mut RNG) {
        match self {
            Content::List(list) => {
                for item in list {
                    if let ContentValue::Element(element) = item {
                        element.give_ids(rng);
                    }
                }
            }
            Content::Value(ContentValue::Element(element)) => element.give_ids(rng),

            Content::Empty => {}
            Content::Value(ContentValue::Raw(_)) => {}
            Content::Value(ContentValue::Text(_)) => {}
            Content::Value(ContentValue::State(_)) => {}
        }
    }

    pub(crate) fn is_reactive(&self) -> bool {
        match self {
            Content::Empty => false,
            Content::Value(value) => value.is_reactive(),
            Content::List(list) => list.iter().any(ContentValue::is_reactive),
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

                /// Add a group as a ReactiveDescriptor
                fn add_group<'a, 'b>(
                    reactivity: &'b mut Reactivity<'a>,
                    group: Vec<&'a ContentValue>,
                    group_id: u32,
                    element_id: Option<RandomId>,
                ) where
                    'a: 'b,
                {
                    let Some(id) = element_id else { return };

                    let state_descriptors =
                        group.iter().filter_map(|c| c.state()).collect::<Vec<_>>();
                    let content = group
                        .iter()
                        .map(|content| match content {
                            ContentValue::Raw(text) => ReactiveContent::Text(text.into()),
                            ContentValue::Text(text) => ReactiveContent::Text(
                                html_escape::encode_script_single_quoted_text(text),
                            ),
                            ContentValue::State(descriptor) => ReactiveContent::Var(
                                state_descriptors.iter().position(|s| *s == descriptor).expect("states always includes all the states that appear in the group"),
                            ),
                            _ => unreachable!("group only contains Raw, Text, and State"),
                        })
                        .collect();

                    reactivity.add_element_content(ElementContentReactivityDescriptor {
                        element_id: id,
                        child_node_idx: Some(group_id),
                        content,
                        state_descriptors,
                    });
                }

                let mut group: Option<Vec<&ContentValue>> = None;
                let mut group_id: u32 = 0;
                for content in list {
                    if content.is_text() {
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

                        if let ContentValue::Element(element) = content {
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
            Content::Value(ContentValue::State(desc)) => {
                let Some(id) = element_id else { return };

                reactivity.add_element_content(ElementContentReactivityDescriptor {
                    element_id: id,
                    child_node_idx: None,
                    state_descriptors: vec![desc],
                    content: vec![ReactiveContent::Var(0)],
                });
            }
            Content::Value(ContentValue::Element(element)) => element.reactivity(reactivity),

            Content::Empty => {}
            Content::Value(ContentValue::Raw(_)) => {}
            Content::Value(ContentValue::Text(_)) => {}
        }
    }

    pub(crate) fn render(&self, output: &mut String) {
        match self {
            Self::Empty => {}
            Self::Value(value) => value.render(output),
            Self::List(list) => {
                for item in list {
                    item.render(output);
                }
            }
        }
    }
}

impl From<()> for Content {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}
impl From<String> for ContentValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl<'a> From<&'a str> for ContentValue {
    fn from(value: &'a str) -> Self {
        Self::Text(value.to_string())
    }
}
impl From<Element> for ContentValue {
    fn from(element: Element) -> Self {
        Self::Element(Box::new(element))
    }
}
impl<T> From<State<T>> for ContentValue
where
    T: Clone + Display + Send + Sync + 'static,
{
    fn from(value: State<T>) -> Self {
        Self::State(value.into())
    }
}

impl<T> From<T> for Content
where
    ContentValue: From<T>,
{
    fn from(value: T) -> Self {
        Self::Value(value.into())
    }
}
impl From<Vec<ContentValue>> for Content {
    fn from(value: Vec<ContentValue>) -> Self {
        Self::List(value)
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
            Content::List(vec![ContentValue::Raw("hey".to_string())]),
            Content::Value(ContentValue::Raw("hey".to_string()))
        );
        run!(
            Content::List(vec![
                ContentValue::Raw("hey".to_string()),
                ContentValue::Text("hi".to_string())
            ]),
            Content::Value(ContentValue::Raw("heyhi".to_string()))
        );
    }
}

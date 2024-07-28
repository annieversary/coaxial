use crate::{random_id::RandomId, reactive_js::Reactivity};

use super::Attribute;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Attributes {
    // TODO change this to a HashMap
    // then on multiple insert we group them into a list
    pub(crate) list: Vec<(String, Attribute)>,
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
        for (i, (key, attr)) in self.list.iter().enumerate() {
            output.push_str(key);

            if matches!(attr, Attribute::Empty) {
                continue;
            }

            output.push_str("=\"");
            attr.render(output);
            output.push('"');

            if i + 1 != self.list.len() {
                output.push(' ');
            }
        }
    }

    pub(crate) fn reactivity<'a, 'b>(
        &'a self,
        element_id: Option<RandomId>,
        reactivity: &'b mut Reactivity<'a>,
    ) where
        'a: 'b,
    {
        for (key, attr) in &self.list {
            attr.reactivity(element_id, key, reactivity);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_can_render_one_attribute() {
        let attrs = attrs!(
            "hi" => "hey",
        );

        let mut output = String::new();
        attrs.render(&mut output);

        // doesn't have an extra space at the end
        assert_eq!(output, "hi=\"hey\"");
    }

    #[test]
    fn test_can_render_multiple_attributes() {
        let attrs = attrs!(
            "onclick" => "hey",
            "data-something" => "wow",
        );

        let mut output = String::new();
        attrs.render(&mut output);

        // has a space between the two attributes, but not at the end
        assert_eq!(output, "onclick=\"hey\" data-something=\"wow\"");
    }
}

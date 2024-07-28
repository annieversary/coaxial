use std::collections::HashMap;

use crate::{random_id::RandomId, reactive_js::Reactivity};

use super::Attribute;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Attributes {
    attributes: HashMap<String, Attribute>,
}

impl Attributes {
    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }

    pub fn insert(&mut self, key: impl ToString, attribute: impl Into<Attribute>) {
        let key = key.to_string();

        // HTML doesn't allow repeated attribute keys.
        // Browsers take the first one and ignore all the rest, so we'll throw an error.
        // https://stackoverflow.com/a/43859478
        debug_assert!(
            !self.attributes.contains_key(&key),
            "trying to override attribute {}",
            key
        );

        // TODO we can consider merging class and styles, but idk

        self.attributes.insert(key, attribute.into());
    }

    pub(crate) fn is_reactive(&self) -> bool {
        self.attributes.values().any(Attribute::is_reactive)
    }

    pub(crate) fn optimize(&mut self) {
        for value in self.attributes.values_mut() {
            value.optimize();
        }
    }

    pub(crate) fn render(&self, output: &mut String) {
        #[cfg(debug_assertions)]
        let iter = {
            let mut v = Vec::from_iter(self.attributes.iter());
            v.sort_by_key(|a| a.0);
            v.into_iter()
        };
        #[cfg(not(debug_assertions))]
        let iter = self.attributes.iter();

        for (i, (key, attr)) in iter.enumerate() {
            output.push_str(key);

            if matches!(attr, Attribute::Empty) {
                continue;
            }

            output.push_str("=\"");
            attr.render(output);
            output.push('"');

            if i + 1 != self.attributes.len() {
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
        for (key, attr) in &self.attributes {
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
    fn test_can_render_list() {
        let attrs = attrs!(
            "greeting" => ("hello", "world"),
        );

        let mut output = String::new();
        attrs.render(&mut output);

        // doesn't have an extra space at the end
        assert_eq!(output, "greeting=\"helloworld\"");
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
        assert_eq!("data-something=\"wow\" onclick=\"hey\"", output);
    }
}

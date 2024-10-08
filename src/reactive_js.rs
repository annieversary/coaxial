use std::{borrow::Cow, collections::HashMap, fmt::Write};

use crate::{html::StateDescriptor, random_id::RandomId};

#[derive(Default)]
pub(crate) struct Reactivity<'a> {
    descriptors: Vec<ReactivityDescriptor<'a>>,

    state_field_initial_values: HashMap<&'a str, &'a str>,
}

impl<'a> Reactivity<'a> {
    pub(crate) fn add(&mut self, descriptor: ReactivityDescriptor<'a>) {
        for state_descriptor in &descriptor.state_descriptors {
            self.register_state(state_descriptor);
        }
        self.descriptors.push(descriptor);
    }

    fn register_state(&mut self, state_descriptor: &'a StateDescriptor) {
        self.state_field_initial_values
            .insert(&state_descriptor.state_id, &state_descriptor.display);
    }

    pub(crate) fn script(&self) -> String {
        let mut output = String::new();

        for descriptor in &self.descriptors {
            descriptor.script(&mut output);
        }

        self.state_field_initial_values_script(&mut output);

        output
    }

    fn state_field_initial_values_script(&self, output: &mut String) {
        for (key, value) in &self.state_field_initial_values {
            write!(output, "window.Coaxial.state['{key}'] = '{value}';").unwrap()
        }
    }
}

pub(crate) struct ReactivityDescriptor<'a> {
    /// Coaxial Id of the element this descriptor applies to
    pub(crate) element_id: RandomId,
    /// Index of `childNodes` to change in this descriptor.
    /// If None, this descriptor applies to the full element.
    pub(crate) child_node_idx: Option<u32>,

    pub(crate) target: Target<'a>,

    pub(crate) state_descriptors: Vec<&'a StateDescriptor>,
    pub(crate) content: Vec<Content<'a>>,
}

impl<'a> ReactivityDescriptor<'a> {
    fn script(&self, output: &mut String) {
        output.push_str("window.Coaxial.onStateChange(['");

        let state_count = self.state_descriptors.len();
        for (i, state_desc) in self.state_descriptors.iter().enumerate() {
            output.push_str(&state_desc.state_id);

            if state_count == i + 1 {
                output.push('\'');
            } else {
                output.push_str("','");
            }
        }

        output.push_str("], (");

        for i in 0..state_count {
            output.push('v');
            output.push_str(&i.to_string());

            if state_count != i + 1 {
                output.push(',');
            }
        }

        output.push_str(") => { if (el = document.querySelector('[coax-id=\"");
        self.element_id.fmt(output).unwrap();
        output.push_str("\"]')) ");

        if let Some(child_node_idx) = self.child_node_idx {
            write!(output, "if (el = el.childNodes[{}]) ", child_node_idx).unwrap();
        }

        match self.target {
            Target::TextContent => output.push_str("el.textContent = "),
            Target::Attribute(key) => {
                output.push_str("el.setAttribute('");
                output.push_str(key);
                output.push_str("', ");
            }
        }

        if self.content.len() == 1 {
            self.content.first().unwrap().script(output);
        } else {
            output.push('[');
            for (i, item) in self.content.iter().enumerate() {
                item.script(output);
                if i + 1 != self.content.len() {
                    output.push(',');
                }
            }
            output.push_str("].join('')");
        }

        if matches!(self.target, Target::Attribute(_)) {
            output.push(')');
        }
        output.push_str("; });");

        #[cfg(debug_assertions)]
        output.push('\n');
    }
}

pub(crate) enum Target<'a> {
    TextContent,
    Attribute(&'a str),
}

pub(crate) enum Content<'a> {
    /// Plain text
    Text(Cow<'a, str>),
    /// Index into the state_ids array
    Var(usize),
}

impl<'a> Content<'a> {
    fn script(&self, output: &mut String) {
        match self {
            Content::Text(text) => write!(output, "'{}'", text).unwrap(),
            Content::Var(idx) => write!(output, "v{}", idx).unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_script() {
        let state_desc = StateDescriptor {
            display: "value".to_string(),
            state_id: "state1".to_string(),
        };
        let desc = ReactivityDescriptor {
            element_id: RandomId::from_str("aaaabbbb"),
            child_node_idx: None,
            state_descriptors: vec![&state_desc],
            content: vec![Content::Var(0)],
            target: Target::TextContent,
        };

        let mut output = String::new();
        desc.script(&mut output);

        assert_eq!("window.Coaxial.onStateChange(['state1'], (v0) => { if (el = document.querySelector('[coax-id=\"aaaabbbb\"]')) el.textContent = v0; });\n", output);
    }

    #[test]
    fn test_setting_attribute() {
        let state_desc = StateDescriptor {
            display: "value".to_string(),
            state_id: "state1".to_string(),
        };
        let desc = ReactivityDescriptor {
            element_id: RandomId::from_str("aaaabbbb"),
            child_node_idx: None,
            state_descriptors: vec![&state_desc],
            content: vec![Content::Var(0)],
            target: Target::Attribute("my-attr"),
        };

        let mut output = String::new();
        desc.script(&mut output);

        assert_eq!("window.Coaxial.onStateChange(['state1'], (v0) => { if (el = document.querySelector('[coax-id=\"aaaabbbb\"]')) el.setAttribute('my-attr', v0); });\n", output);
    }

    #[test]
    fn test_child_node() {
        let state_desc = StateDescriptor {
            display: "value".to_string(),
            state_id: "state1".to_string(),
        };
        let desc = ReactivityDescriptor {
            element_id: RandomId::from_str("aaaabbbb"),
            child_node_idx: Some(22),
            state_descriptors: vec![&state_desc],
            content: vec![Content::Text("hey".into())],
            target: Target::TextContent,
        };

        let mut output = String::new();
        desc.script(&mut output);

        assert_eq!("window.Coaxial.onStateChange(['state1'], (v0) => { if (el = document.querySelector('[coax-id=\"aaaabbbb\"]')) if (el = el.childNodes[22]) el.textContent = 'hey'; });\n", output);
    }

    #[test]
    fn test_multiple_content() {
        let state_desc = StateDescriptor {
            display: "value".to_string(),
            state_id: "state1".to_string(),
        };
        let desc = ReactivityDescriptor {
            element_id: RandomId::from_str("aaaabbbb"),
            child_node_idx: None,
            state_descriptors: vec![&state_desc],
            content: vec![
                Content::Text("hey".into()),
                Content::Var(0),
                Content::Text("world".into()),
            ],
            target: Target::TextContent,
        };

        let mut output = String::new();
        desc.script(&mut output);

        assert_eq!("window.Coaxial.onStateChange(['state1'], (v0) => { if (el = document.querySelector('[coax-id=\"aaaabbbb\"]')) el.textContent = ['hey',v0,'world'].join(''); });\n", output);
    }
    #[test]
    fn test_multiple_content_attribute() {
        let state_desc = StateDescriptor {
            display: "value".to_string(),
            state_id: "state1".to_string(),
        };
        let desc = ReactivityDescriptor {
            element_id: RandomId::from_str("aaaabbbb"),
            child_node_idx: None,
            state_descriptors: vec![&state_desc],
            content: vec![
                Content::Text("hey".into()),
                Content::Var(0),
                Content::Text("world".into()),
            ],
            target: Target::Attribute("my-attr"),
        };

        let mut output = String::new();
        desc.script(&mut output);

        assert_eq!("window.Coaxial.onStateChange(['state1'], (v0) => { if (el = document.querySelector('[coax-id=\"aaaabbbb\"]')) el.setAttribute('my-attr', ['hey',v0,'world'].join('')); });\n", output);
    }

    #[test]
    fn test_multiple_states() {
        let state_desc_1 = StateDescriptor {
            display: "value1".to_string(),
            state_id: "state1".to_string(),
        };
        let state_desc_2 = StateDescriptor {
            display: "value2".to_string(),
            state_id: "state2".to_string(),
        };
        let desc = ReactivityDescriptor {
            element_id: RandomId::from_str("aaaabbbb"),
            child_node_idx: None,
            state_descriptors: vec![&state_desc_1, &state_desc_2],
            content: vec![
                Content::Var(1),
                Content::Text("um".into()),
                Content::Var(0),
                Content::Text("wow".into()),
                Content::Var(1),
                Content::Var(0),
                Content::Var(1),
            ],
            target: Target::TextContent,
        };

        let mut output = String::new();
        desc.script(&mut output);

        assert_eq!("window.Coaxial.onStateChange(['state1','state2'], (v0,v1) => { if (el = document.querySelector('[coax-id=\"aaaabbbb\"]')) el.textContent = [v1,'um',v0,'wow',v1,v0,v1].join(''); });\n", output);
    }
}

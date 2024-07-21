use super::Attribute;

macro_rules! push_strs {
    ( $output:ident => $($vals:expr),* $(,)? ) => {
        $(
            $output.push_str($vals);
        )*
    };
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Attributes {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_render_attributes() {
        let attrs = attrs!(
            "onclick" => Attribute::Text("hey".to_string()),
        );

        let mut output = String::new();
        attrs.render(&mut output);

        assert_eq!(output, "onclick=\"hey\"");
    }
}

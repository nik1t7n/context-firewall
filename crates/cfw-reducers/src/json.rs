use serde_json::Value;

use crate::{Reducer, Reduction};

pub struct JsonReducer;

impl Reducer for JsonReducer {
    fn name(&self) -> &'static str {
        "json"
    }

    fn reduce(&self, input: &str) -> Reduction {
        match serde_json::from_str::<Value>(input) {
            Ok(value) => {
                let mut output = String::new();
                output.push_str("[context-firewall: json shape]\n");
                render_value(&mut output, "$", &value, 0);
                Reduction {
                    reducer: self.name().to_string(),
                    output,
                    omitted: true,
                    notes: vec![
                        "returned JSON structure, small scalar samples, and collection sizes"
                            .to_string(),
                    ],
                }
            }
            Err(error) => {
                let mut output = String::new();
                output.push_str(&format!("[context-firewall: invalid json; {}]\n", error));
                for line in input.lines().take(120) {
                    output.push_str(line);
                    output.push('\n');
                }
                let total = input.lines().count();
                if total > 120 {
                    output.push_str(&format!(
                        "[context-firewall: omitted {} additional lines]\n",
                        total - 120
                    ));
                }
                Reduction {
                    reducer: self.name().to_string(),
                    output,
                    omitted: total > 120,
                    notes: vec!["input did not parse as JSON; returned capped text".to_string()],
                }
            }
        }
    }
}

fn render_value(output: &mut String, path: &str, value: &Value, depth: usize) {
    const MAX_DEPTH: usize = 6;
    const MAX_OBJECT_FIELDS: usize = 40;
    const MAX_ARRAY_SAMPLES: usize = 3;

    if depth > MAX_DEPTH {
        output.push_str(&format!("{path}: ...\n"));
        return;
    }

    match value {
        Value::Null => output.push_str(&format!("{path}: null\n")),
        Value::Bool(value) => output.push_str(&format!("{path}: bool({value})\n")),
        Value::Number(value) => output.push_str(&format!("{path}: number({value})\n")),
        Value::String(value) => {
            let sample = truncate(value, 80);
            output.push_str(&format!(
                "{path}: string(len={}, sample={sample:?})\n",
                value.len()
            ));
        }
        Value::Array(values) => {
            output.push_str(&format!("{path}: array(len={})\n", values.len()));
            for (idx, item) in values.iter().take(MAX_ARRAY_SAMPLES).enumerate() {
                render_value(output, &format!("{path}[{idx}]"), item, depth + 1);
            }
            if values.len() > MAX_ARRAY_SAMPLES {
                output.push_str(&format!(
                    "{path}: ... {} additional array items\n",
                    values.len() - MAX_ARRAY_SAMPLES
                ));
            }
        }
        Value::Object(map) => {
            output.push_str(&format!("{path}: object(keys={})\n", map.len()));
            for (idx, (key, item)) in map.iter().enumerate() {
                if idx >= MAX_OBJECT_FIELDS {
                    output.push_str(&format!(
                        "{path}: ... {} additional object keys\n",
                        map.len() - MAX_OBJECT_FIELDS
                    ));
                    break;
                }
                render_value(output, &format!("{path}.{}", key), item, depth + 1);
            }
        }
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
}

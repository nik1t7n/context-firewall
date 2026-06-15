use crate::{Reducer, Reduction};

pub struct GitReducer;

impl Reducer for GitReducer {
    fn name(&self) -> &'static str {
        "git"
    }

    fn reduce(&self, input: &str) -> Reduction {
        const MAX_INTERESTING_LINES: usize = 180;
        let mut kept = Vec::new();
        let lines: Vec<&str> = input.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let interesting = line.starts_with("diff --git ")
                || line.starts_with("index ")
                || line.starts_with("--- ")
                || line.starts_with("+++ ")
                || line.starts_with("@@ ")
                || line.starts_with("<<<<<<<")
                || line.starts_with("=======")
                || line.starts_with(">>>>>>>")
                || (line.starts_with('+') && !line.starts_with("+++"))
                || (line.starts_with('-') && !line.starts_with("---"));

            if interesting {
                kept.push((idx + 1, *line));
            }
            if kept.len() >= MAX_INTERESTING_LINES {
                break;
            }
        }

        if kept.len() == lines.len() {
            return Reduction {
                reducer: self.name().to_string(),
                output: input.to_string(),
                omitted: false,
                notes: vec![],
            };
        }

        let mut output = String::new();
        for (line_no, line) in &kept {
            output.push_str(&format!("{line_no}: {line}\n"));
        }

        let omitted = lines.len().saturating_sub(kept.len());
        if omitted > 0 {
            output.push_str(&format!(
                "[context-firewall: omitted {omitted} git context lines; use cfw show <span> for full diff]\n"
            ));
        }

        Reduction {
            reducer: self.name().to_string(),
            output,
            omitted: omitted > 0,
            notes: vec![
                "preserved diff headers, hunk headers, changed lines, and conflict markers"
                    .to_string(),
            ],
        }
    }
}

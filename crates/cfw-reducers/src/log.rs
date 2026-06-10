use regex::Regex;

use crate::{Reducer, Reduction};

pub struct LogReducer;

impl Reducer for LogReducer {
    fn name(&self) -> &'static str {
        "log"
    }

    fn reduce(&self, input: &str) -> Reduction {
        const EDGE_LINES: usize = 20;
        const CONTEXT: usize = 2;
        const MAX_LINES: usize = 240;

        let signal_re = Regex::new(
            r"(?i)\b(error|fatal|panic|exception|traceback|warn|warning|failed|timeout|denied|refused|unauthorized|segfault)\b",
        )
        .expect("valid log signal regex");
        let lines: Vec<&str> = input.lines().collect();

        if lines.len() <= EDGE_LINES * 2 {
            return Reduction {
                reducer: self.name().to_string(),
                output: input.to_string(),
                omitted: false,
                notes: vec![],
            };
        }

        let mut keep = vec![false; lines.len()];
        for idx in 0..lines.len() {
            if idx < EDGE_LINES || idx + EDGE_LINES >= lines.len() || signal_re.is_match(lines[idx])
            {
                let start = idx.saturating_sub(CONTEXT);
                let end = (idx + CONTEXT + 1).min(lines.len());
                for slot in keep.iter_mut().take(end).skip(start) {
                    *slot = true;
                }
            }
        }

        let mut kept = Vec::new();
        for (idx, should_keep) in keep.iter().enumerate() {
            if *should_keep {
                kept.push((idx + 1, lines[idx]));
            }
            if kept.len() >= MAX_LINES {
                break;
            }
        }

        render_sparse(
            self.name(),
            &kept,
            lines.len(),
            "preserved log edges plus severity/error context",
        )
    }
}

fn render_sparse(name: &str, kept: &[(usize, &str)], total_lines: usize, note: &str) -> Reduction {
    let mut output = String::new();
    let mut last_line = 0usize;
    for (line_no, line) in kept {
        if *line_no > last_line + 1 {
            output.push_str(&format!(
                "[context-firewall: omitted lines {}-{}]\n",
                last_line + 1,
                line_no - 1
            ));
        }
        output.push_str(&format!("{line_no}: {line}\n"));
        last_line = *line_no;
    }
    if last_line < total_lines {
        output.push_str(&format!(
            "[context-firewall: omitted lines {}-{}]\n",
            last_line + 1,
            total_lines
        ));
    }

    Reduction {
        reducer: name.to_string(),
        output,
        omitted: kept.len() < total_lines,
        notes: vec![note.to_string()],
    }
}

use regex::Regex;

use crate::{Reducer, Reduction};

pub struct TestOutputReducer;

impl Reducer for TestOutputReducer {
    fn name(&self) -> &'static str {
        "test-output"
    }

    fn reduce(&self, input: &str) -> Reduction {
        let failure_re = Regex::new(
            r"(?i)(fail|failed|failure|error|panic|assert|AssertionError|Traceback|expected|actual)",
        )
        .expect("valid failure regex");
        let summary_re =
            Regex::new(r"(?i)(test result:|passed|failed|errors?|failures?|collected)")
                .expect("valid summary regex");

        let mut kept = Vec::new();
        let lines: Vec<&str> = input.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let near_failure = failure_re.is_match(line)
                || summary_re.is_match(line)
                || idx < 20
                || idx + 20 >= lines.len();
            if near_failure {
                kept.push((idx + 1, *line));
            }
        }

        kept.dedup_by_key(|(line_no, _)| *line_no);

        let mut output = String::new();
        let mut last_line = 0usize;
        for (line_no, line) in &kept {
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

        let omitted = kept.len() < lines.len();
        Reduction {
            reducer: self.name().to_string(),
            output,
            omitted,
            notes: vec![
                "preserved head/tail, failure-looking lines, and summary-looking lines".to_string(),
            ],
        }
    }
}

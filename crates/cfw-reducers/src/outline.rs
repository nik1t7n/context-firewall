use regex::Regex;

use crate::{Reducer, Reduction};

pub struct OutlineReducer;

impl Reducer for OutlineReducer {
    fn name(&self) -> &'static str {
        "outline"
    }

    fn reduce(&self, input: &str) -> Reduction {
        const MAX_LINES: usize = 220;

        let heading_re = Regex::new(
            r#"^\s{0,8}(#{1,6}\s+.+|[-*]\s+\[[ xX]\]\s+.+|\[\[?.+\]\]?|name\s*=|version\s*=)"#,
        )
        .expect("valid heading regex");
        let code_re = Regex::new(
            r"^\s{0,8}((pub\s+)?(async\s+)?fn\s+\w+|class\s+\w+|def\s+\w+|function\s+\w+|export\s+(async\s+)?function\s+\w+|const\s+\w+\s*=|interface\s+\w+|type\s+\w+\s*=|(pub\s+)?struct\s+\w+|(pub\s+)?enum\s+\w+|impl\b|mod\s+\w+)",
        )
        .expect("valid outline regex");
        let import_re = Regex::new(
            r"^\s{0,4}(use\s+.+;|import\s+.+|from\s+\S+\s+import\s+.+|require\(.+\)|package\s+\w+)",
        )
        .expect("valid import regex");

        let lines: Vec<&str> = input.lines().collect();
        let mut kept = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            if heading_re.is_match(line) || code_re.is_match(line) || import_re.is_match(line) {
                kept.push((idx + 1, *line));
            }
            if kept.len() >= MAX_LINES {
                break;
            }
        }

        if kept.is_empty() {
            for (idx, line) in lines.iter().take(80).enumerate() {
                kept.push((idx + 1, *line));
            }
        }

        let mut output = String::new();
        output.push_str("[context-firewall: file outline]\n");
        output.push_str(&format!("raw lines: {}\n\n", lines.len()));
        for (line_no, line) in &kept {
            output.push_str(&format!("{line_no}: {line}\n"));
        }
        if kept.len() < lines.len() {
            output.push_str(&format!(
                "[context-firewall: omitted {} non-outline lines]\n",
                lines.len() - kept.len()
            ));
        }

        Reduction {
            reducer: self.name().to_string(),
            output,
            omitted: kept.len() < lines.len(),
            notes: vec!["preserved headings, imports, and top-level code declarations".to_string()],
        }
    }
}

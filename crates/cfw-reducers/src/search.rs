use std::collections::BTreeMap;

use crate::{Reducer, Reduction};

pub struct SearchReducer;

impl Reducer for SearchReducer {
    fn name(&self) -> &'static str {
        "search"
    }

    fn reduce(&self, input: &str) -> Reduction {
        const MAX_FILES: usize = 80;
        const MAX_MATCHES_PER_FILE: usize = 6;
        const MAX_PATHS: usize = 80;

        let lines: Vec<&str> = input.lines().collect();
        if lines.len() <= 120 {
            return Reduction {
                reducer: self.name().to_string(),
                output: input.to_string(),
                omitted: false,
                notes: vec![],
            };
        }

        let mut matches_by_file: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut parsed_matches = 0usize;
        for line in &lines {
            if let Some((path, rest)) = split_search_match(line) {
                parsed_matches += 1;
                let entries = matches_by_file.entry(path).or_default();
                if entries.len() < MAX_MATCHES_PER_FILE {
                    entries.push(rest);
                }
            }
        }

        if !matches_by_file.is_empty() {
            let mut output = String::new();
            output.push_str("[context-firewall: search summary]\n");
            output.push_str(&format!("files matched: {}\n", matches_by_file.len()));
            output.push_str(&format!("raw match lines: {}\n\n", parsed_matches));

            for (idx, (path, entries)) in matches_by_file.iter().enumerate() {
                if idx >= MAX_FILES {
                    output.push_str(&format!(
                        "[context-firewall: omitted {} additional files]\n",
                        matches_by_file.len() - MAX_FILES
                    ));
                    break;
                }
                output.push_str(path);
                output.push('\n');
                for entry in entries {
                    output.push_str("  ");
                    output.push_str(entry);
                    output.push('\n');
                }
            }

            return Reduction {
                reducer: self.name().to_string(),
                output,
                omitted: parsed_matches > matches_by_file.values().map(Vec::len).sum::<usize>()
                    || matches_by_file.len() > MAX_FILES,
                notes: vec![
                    "grouped search matches by file and capped matches per file".to_string(),
                ],
            };
        }

        let mut output = String::new();
        output.push_str("[context-firewall: path listing summary]\n");
        output.push_str(&format!("raw lines: {}\n\n", lines.len()));
        for line in lines.iter().take(MAX_PATHS) {
            output.push_str(line);
            output.push('\n');
        }
        if lines.len() > MAX_PATHS {
            output.push_str(&format!(
                "[context-firewall: omitted {} additional paths]\n",
                lines.len() - MAX_PATHS
            ));
        }

        Reduction {
            reducer: self.name().to_string(),
            output,
            omitted: lines.len() > MAX_PATHS,
            notes: vec!["capped large path/listing output".to_string()],
        }
    }
}

fn split_search_match(line: &str) -> Option<(&str, &str)> {
    let (path, after_path) = line.split_once(':')?;
    if path.is_empty() || after_path.is_empty() {
        return None;
    }
    if path.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(("[single-file matches]", line));
    }
    let first = after_path.split(':').next().unwrap_or_default();
    if first.chars().all(|ch| ch.is_ascii_digit()) {
        Some((path, after_path))
    } else {
        None
    }
}

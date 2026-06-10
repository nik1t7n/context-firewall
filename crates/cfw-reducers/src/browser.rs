use std::collections::BTreeMap;

use crate::{Reducer, Reduction};

pub struct BrowserSnapshotReducer;

impl Reducer for BrowserSnapshotReducer {
    fn name(&self) -> &'static str {
        "browser-snapshot"
    }

    fn reduce(&self, input: &str) -> Reduction {
        const MAX_TOTAL_LINES: usize = 80;
        const MAX_IMPORTANT_LINES: usize = 45;
        const EDGE_LINES: usize = 12;

        let lines: Vec<&str> = input.lines().collect();
        if lines.len() <= MAX_TOTAL_LINES {
            return Reduction {
                reducer: self.name().to_string(),
                output: input.to_string(),
                omitted: false,
                notes: vec![],
            };
        }

        let mut role_counts = BTreeMap::<String, usize>::new();
        let mut important = Vec::<String>::new();
        let mut diagnostics = Vec::<String>::new();
        for line in &lines {
            if let Some(role) = aria_role(line) {
                *role_counts.entry(role.to_string()).or_insert(0) += 1;
            }
            if is_browser_diagnostic(line) {
                push_unique(&mut diagnostics, line.trim(), 20);
                continue;
            }
            if is_important_snapshot_line(line) {
                push_unique(&mut important, line.trim(), MAX_IMPORTANT_LINES);
            }
        }

        let mut output = String::new();
        output.push_str("[context-firewall: browser snapshot summary]\n");
        output.push_str(&format!("raw lines: {}\n", lines.len()));
        if !role_counts.is_empty() {
            output.push_str("roles:");
            for (role, count) in &role_counts {
                output.push_str(&format!(" {role}={count}"));
            }
            output.push('\n');
        }

        if !diagnostics.is_empty() {
            output.push_str("\ndiagnostics:\n");
            for line in diagnostics {
                output.push_str("- ");
                output.push_str(&line);
                output.push('\n');
            }
        }

        if !important.is_empty() {
            output.push_str("\nkey accessible nodes:\n");
            for line in important {
                output.push_str("- ");
                output.push_str(&line);
                output.push('\n');
            }
        }

        output.push_str("\nhead:\n");
        for line in lines.iter().take(EDGE_LINES) {
            output.push_str(line);
            output.push('\n');
        }

        let omitted = lines.len().saturating_sub(EDGE_LINES * 2);
        output.push_str(&format!(
            "\n[context-firewall: omitted {omitted} middle browser snapshot lines]\n\n"
        ));

        output.push_str("tail:\n");
        for line in lines.iter().skip(lines.len().saturating_sub(EDGE_LINES)) {
            output.push_str(line);
            output.push('\n');
        }

        Reduction {
            reducer: self.name().to_string(),
            output,
            omitted: true,
            notes: vec![
                "summarized browser/accessibility snapshot roles and key interactive nodes"
                    .to_string(),
            ],
        }
    }
}

fn push_unique(lines: &mut Vec<String>, line: &str, limit: usize) {
    if lines.len() >= limit || lines.iter().any(|existing| existing == line) {
        return;
    }
    lines.push(line.to_string());
}

fn is_browser_diagnostic(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("error")
        || lower.contains("warning")
        || lower.contains("failed")
        || lower.contains("timeout")
        || lower.starts_with("url:")
        || lower.starts_with("title:")
        || lower.starts_with("page:")
}

fn is_important_snapshot_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("alert") || lower.contains("dialog") {
        return true;
    }
    matches!(
        aria_role(line),
        Some(
            "alert"
                | "button"
                | "checkbox"
                | "combobox"
                | "dialog"
                | "heading"
                | "link"
                | "menuitem"
                | "option"
                | "radio"
                | "searchbox"
                | "tab"
                | "textbox"
        )
    )
}

fn aria_role(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("- ")?;
    let role = rest
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '"' | '/' | ':' | '[')
        })
        .next()
        .unwrap_or("");
    if role.is_empty() || role.starts_with('/') {
        None
    } else {
        Some(role)
    }
}

use std::path::Path;

use anyhow::{Context, Result};

const START_MARKER: &str = "<!-- context-firewall:start -->";
const END_MARKER: &str = "<!-- context-firewall:end -->";

pub fn wrapper_snippet() -> String {
    format!(
        r#"{START_MARKER}
## Context Firewall

Use `cfw run -- <command>` for commands likely to produce large output:
test suites, build logs, git diffs, search results, file dumps, generated files,
browser snapshots, and logs.

When Context Firewall returns a span handle like `cfw://span/<id>`, request exact
lines with `cfw show <id> --lines A:B` instead of rerunning the command.

Context Firewall wrapper mode is advisory. Hook-native replacement is not active
until `cfw doctor codex` reports `hook_replacement_verified: true`.
{END_MARKER}
"#
    )
}

pub fn write_wrapper_snippet(path: &Path) -> Result<InstallOutcome> {
    let snippet = wrapper_snippet();
    let existing = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("could not read {}", path.display())),
    };

    if existing.contains(START_MARKER) && existing.contains(END_MARKER) {
        return Ok(InstallOutcome::AlreadyPresent);
    }

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    if !next.is_empty() {
        next.push('\n');
    }
    next.push_str(&snippet);

    std::fs::write(path, next).with_context(|| format!("could not write {}", path.display()))?;
    Ok(InstallOutcome::Written)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    Written,
    AlreadyPresent,
}

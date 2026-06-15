use std::path::Path;

use anyhow::{Context, Result, bail};

const START_MARKER: &str = "<!-- context-firewall:start -->";
const END_MARKER: &str = "<!-- context-firewall:end -->";

pub fn wrapper_snippet() -> String {
    format!(
        r#"{START_MARKER}
## Context Firewall

Use Context Firewall for noisy terminal output.

- Prefer `cfw run -- <command>` for tests, builds, git diffs, search results,
  file dumps, generated files, browser snapshots, and logs.
- If MCP tools are available, prefer `cfw_run`, `cfw_show`, `cfw_spans`, and
  `cfw_receipt`.
- When Context Firewall returns a span handle like `cfw://span/<id>`, retrieve
  exact lines with `cfw show <id> --lines A:B` instead of rerunning the command.

Keep raw evidence on disk. Keep agent context clean.
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

    if let Some(range) = marker_range(&existing)? {
        if existing[range.clone()] == snippet {
            return Ok(InstallOutcome::AlreadyPresent);
        }
        let mut next = existing;
        next.replace_range(range, &snippet);
        std::fs::write(path, next)
            .with_context(|| format!("could not write {}", path.display()))?;
        return Ok(InstallOutcome::Written);
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

pub fn inspect_wrapper_snippet(path: &Path) -> Result<InstallOutcome> {
    let existing = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("could not read {}", path.display())),
    };

    if marker_range(&existing)?.is_some() {
        Ok(InstallOutcome::AlreadyPresent)
    } else {
        Ok(InstallOutcome::Written)
    }
}

pub fn uninstall_wrapper_snippet(path: &Path) -> Result<UninstallOutcome> {
    let existing = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(UninstallOutcome::NotFound);
        }
        Err(err) => return Err(err).with_context(|| format!("could not read {}", path.display())),
    };

    let Some(range) = marker_range(&existing)? else {
        return Ok(UninstallOutcome::AlreadyAbsent);
    };

    let mut next = existing;
    next.replace_range(range, "");
    std::fs::write(path, next).with_context(|| format!("could not write {}", path.display()))?;
    Ok(UninstallOutcome::Removed)
}

fn marker_range(content: &str) -> Result<Option<std::ops::Range<usize>>> {
    let start = content.find(START_MARKER);
    let end = content.find(END_MARKER);
    match (start, end) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) if start <= end => {
            let mut range_end = end + END_MARKER.len();
            if content[range_end..].starts_with('\n') {
                range_end += 1;
            }
            if start > 0
                && content[..start].ends_with("\n\n")
                && content[range_end..].starts_with('\n')
            {
                range_end += 1;
            }
            Ok(Some(start..range_end))
        }
        _ => bail!("managed Context Firewall block markers are incomplete or out of order"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    Written,
    AlreadyPresent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallOutcome {
    Removed,
    AlreadyAbsent,
    NotFound,
}

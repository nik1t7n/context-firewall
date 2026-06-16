use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const START_MARKER: &str = "<!-- context-firewall:start -->";
const END_MARKER: &str = "<!-- context-firewall:end -->";
const HOOKS_JSON_RELATIVE_PATH: &str = ".codex/hooks.json";
const HOOK_SCRIPT_RELATIVE_PATH: &str = ".codex/hooks/context_firewall_pre_tool_use.py";

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

pub fn hook_native_paths(project_root: &Path) -> HookNativePaths {
    HookNativePaths {
        hooks_json_path: project_root.join(HOOKS_JSON_RELATIVE_PATH),
        hook_script_path: project_root.join(HOOK_SCRIPT_RELATIVE_PATH),
    }
}

pub fn install_hook_native(project_root: &Path) -> Result<InstallOutcome> {
    let paths = hook_native_paths(project_root);
    let config = hook_native_config(&paths.hook_script_path);
    let script = hook_native_script();

    if let Ok(existing) = std::fs::read_to_string(&paths.hooks_json_path) {
        if existing == config {
            if matches!(
                std::fs::read_to_string(&paths.hook_script_path),
                Ok(existing_script) if existing_script == script
            ) {
                return Ok(InstallOutcome::AlreadyPresent);
            }
            if paths.hook_script_path.exists() {
                bail!(
                    "HookNativeInstallConflict: {} exists and is not managed by Context Firewall",
                    paths.hook_script_path.display()
                );
            }
        } else {
            bail!(
                "HookNativeInstallConflict: {} already exists and is not managed by Context Firewall",
                paths.hooks_json_path.display()
            );
        }
    }

    if let Some(parent) = paths.hooks_json_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    if let Some(parent) = paths.hook_script_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }

    std::fs::write(&paths.hooks_json_path, config)
        .with_context(|| format!("could not write {}", paths.hooks_json_path.display()))?;
    std::fs::write(&paths.hook_script_path, script)
        .with_context(|| format!("could not write {}", paths.hook_script_path.display()))?;
    Ok(InstallOutcome::Written)
}

pub fn inspect_hook_native(project_root: &Path) -> Result<InstallOutcome> {
    let paths = hook_native_paths(project_root);
    let Ok(existing) = std::fs::read_to_string(&paths.hooks_json_path) else {
        return Ok(InstallOutcome::Written);
    };

    if existing != hook_native_config(&paths.hook_script_path) {
        return Ok(InstallOutcome::Written);
    }

    if matches!(
        std::fs::read_to_string(&paths.hook_script_path),
        Ok(existing_script) if existing_script == hook_native_script()
    ) {
        Ok(InstallOutcome::AlreadyPresent)
    } else {
        Ok(InstallOutcome::Written)
    }
}

pub fn preview_install_hook_native(project_root: &Path) -> Result<InstallOutcome> {
    let paths = hook_native_paths(project_root);
    let config = hook_native_config(&paths.hook_script_path);
    let Ok(existing) = std::fs::read_to_string(&paths.hooks_json_path) else {
        return Ok(InstallOutcome::Written);
    };

    if existing != config {
        bail!(
            "HookNativeInstallConflict: {} already exists and is not managed by Context Firewall",
            paths.hooks_json_path.display()
        );
    }

    if matches!(
        std::fs::read_to_string(&paths.hook_script_path),
        Ok(existing_script) if existing_script == hook_native_script()
    ) {
        Ok(InstallOutcome::AlreadyPresent)
    } else if paths.hook_script_path.exists() {
        bail!(
            "HookNativeInstallConflict: {} exists and is not managed by Context Firewall",
            paths.hook_script_path.display()
        );
    } else {
        Ok(InstallOutcome::Written)
    }
}

pub fn uninstall_hook_native(project_root: &Path) -> Result<UninstallOutcome> {
    let paths = hook_native_paths(project_root);
    let config_exists = paths.hooks_json_path.exists();
    let script_exists = paths.hook_script_path.exists();
    if !config_exists && !script_exists {
        return Ok(UninstallOutcome::NotFound);
    }

    let config_is_managed = if config_exists {
        let existing = std::fs::read_to_string(&paths.hooks_json_path)
            .with_context(|| format!("could not read {}", paths.hooks_json_path.display()))?;
        if existing != hook_native_config(&paths.hook_script_path) {
            return Ok(UninstallOutcome::AlreadyAbsent);
        }
        true
    } else {
        false
    };

    let script_is_managed = if script_exists {
        let existing = std::fs::read_to_string(&paths.hook_script_path)
            .with_context(|| format!("could not read {}", paths.hook_script_path.display()))?;
        if existing != hook_native_script() {
            bail!(
                "HookNativeUninstallConflict: {} is not managed by Context Firewall",
                paths.hook_script_path.display()
            );
        }
        true
    } else {
        false
    };

    if config_is_managed {
        std::fs::remove_file(&paths.hooks_json_path)
            .with_context(|| format!("could not remove {}", paths.hooks_json_path.display()))?;
    }

    if script_is_managed {
        std::fs::remove_file(&paths.hook_script_path)
            .with_context(|| format!("could not remove {}", paths.hook_script_path.display()))?;
    }

    Ok(UninstallOutcome::Removed)
}

fn hook_native_config(hook_script_path: &Path) -> String {
    let command = format!(
        "python3 {}",
        shell_quote(&hook_script_path.display().to_string())
    );
    let config = serde_json::json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": command,
                            "timeout": 30,
                            "statusMessage": "Routing noisy Bash through Context Firewall"
                        }
                    ]
                }
            ]
        }
    });
    format!(
        "{}\n",
        serde_json::to_string_pretty(&config).expect("hook config is serializable")
    )
}

fn shell_quote(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn hook_native_script() -> &'static str {
    r#"#!/usr/bin/env python3
import json
import shlex
import sys


def classify(argv):
    if not argv:
        return None
    head = argv[0]
    second = argv[1] if len(argv) > 1 else ""
    joined = " ".join(argv[:3])

    if head == "cfw" or "cfw run" in joined:
        return None
    if head == "git" and second in {"diff", "show", "log"}:
        return "git"
    if head in {"rg", "grep", "ag", "ack"}:
        return "search"
    if head in {"docker", "kubectl"} and "logs" in argv[1:3]:
        return "large_log"
    if head in {"cargo", "pytest", "vitest", "jest", "tsc", "eslint"}:
        return "test_output"
    if head in {"npm", "pnpm", "yarn"} and second in {"test", "run"}:
        return "test_output"
    if head == "terraform" and second == "plan":
        return "test_output"
    if head == "gh" and len(argv) > 2 and argv[1] == "pr" and argv[2] in {"checks", "view"}:
        return "test_output"
    return None


def main():
    payload = json.load(sys.stdin)
    if payload.get("hook_event_name") != "PreToolUse" or payload.get("tool_name") != "Bash":
        return 0

    command = payload.get("tool_input", {}).get("command")
    if not isinstance(command, str) or not command.strip():
        return 0

    try:
        argv = shlex.split(command)
    except ValueError:
        return 0

    kind = classify(argv)
    if kind is None:
        return 0

    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": {
                "command": f"cfw run --kind {kind} -- {command}"
            }
        }
    }))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
"#
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookNativePaths {
    pub hooks_json_path: PathBuf,
    pub hook_script_path: PathBuf,
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CanaryOptions {
    pub evidence_root: PathBuf,
    pub codex_bin: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryResult {
    pub verified: bool,
    pub reason: String,
    pub canary_id: String,
    pub codex_version: Option<String>,
    pub raw_marker: String,
    pub compact_marker: String,
    pub workspace_path: String,
    pub events_path: String,
    pub stderr_path: String,
    pub last_message_path: String,
    pub hook_input_path: String,
    pub hook_output_path: String,
}

pub fn run_output_replacement_canary(options: CanaryOptions) -> Result<CanaryResult> {
    let canary_id = new_canary_id();
    let raw_marker = format!("CFW_RAW_MARKER_{canary_id}");
    let compact_marker = format!("CFW_COMPACT_MARKER_{canary_id}");
    let workspace = options.evidence_root.join(&canary_id).join("workspace");
    fs::create_dir_all(workspace.join(".codex/hooks"))
        .with_context(|| format!("could not create {}", workspace.display()))?;

    let hook_input_path = workspace.join("hook-input.json");
    let hook_output_path = workspace.join("hook-output.json");
    let events_path = workspace.join("codex-events.jsonl");
    let stderr_path = workspace.join("codex-stderr.txt");
    let last_message_path = workspace.join("last-message.json");
    let raw_path = workspace.join("raw-marker.txt");
    let schema_path = workspace.join("schema.json");
    let hook_path = workspace.join(".codex/hooks/post_tool_use_canary.py");

    fs::write(&raw_path, format!("{raw_marker}\n"))
        .with_context(|| format!("could not write {}", raw_path.display()))?;
    fs::write(&schema_path, output_schema())
        .with_context(|| format!("could not write {}", schema_path.display()))?;
    fs::write(
        &hook_path,
        hook_script(
            &raw_marker,
            &compact_marker,
            &hook_input_path,
            &hook_output_path,
        ),
    )
    .with_context(|| format!("could not write {}", hook_path.display()))?;
    let project_hook_config = hook_config(&hook_path);
    fs::write(workspace.join(".codex/config.toml"), &project_hook_config)
        .with_context(|| "could not write project Codex config")?;
    fs::write(workspace.join(".codex/hooks.json"), hook_json(&hook_path))
        .with_context(|| "could not write project Codex hooks.json")?;

    let profile_name = format!("cfw-{}", canary_id.replace('-', "_"));
    let profile_path = codex_home()?.join(format!("{profile_name}.config.toml"));
    fs::write(&profile_path, &project_hook_config)
        .with_context(|| format!("could not write {}", profile_path.display()))?;

    let _ = Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(&workspace)
        .status();

    let codex_version = Command::new(&options.codex_bin)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());

    let prompt = "Run exactly one shell command: cat raw-marker.txt\n\
         Then return only JSON matching the provided schema. \
         Set visible_output to the exact complete output you saw from that shell command result. \
         Set raw_marker_seen true only if the visible command result contained text starting with CFW_RAW_MARKER_. \
         Set compact_marker_seen true only if the visible command result contained text starting with CFW_COMPACT_MARKER_. \
         Do not inspect files in any other way.";

    let mut command = Command::new(&options.codex_bin);
    command
        .current_dir(&workspace)
        .stdin(Stdio::null())
        .arg("exec")
        .arg("--json")
        .arg("--ephemeral")
        .arg("--ignore-user-config")
        .arg("--enable")
        .arg("hooks")
        .arg("--dangerously-bypass-hook-trust")
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg("--skip-git-repo-check")
        .arg("--profile")
        .arg(&profile_name)
        .arg("-C")
        .arg(&workspace)
        .arg("-o")
        .arg(&last_message_path)
        .arg("--output-schema")
        .arg(&schema_path)
        .arg("-c")
        .arg("features.hooks=true")
        .arg("-c")
        .arg(cli_hook_override(&hook_path))
        .arg("-c")
        .arg("model_reasoning_effort=\"low\"");
    if let Some(model) = options.model {
        command.arg("--model").arg(model);
    }
    command.arg(prompt);

    let output_result = command.output();
    let _ = fs::remove_file(&profile_path);
    let output = output_result.with_context(|| format!("could not run {}", options.codex_bin))?;
    fs::write(&events_path, &output.stdout)
        .with_context(|| format!("could not write {}", events_path.display()))?;
    fs::write(&stderr_path, &output.stderr)
        .with_context(|| format!("could not write {}", stderr_path.display()))?;

    let hook_input = read_optional(&hook_input_path)?;
    let hook_output = read_optional(&hook_output_path)?;
    let last_message = read_optional(&last_message_path)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    let hook_saw_raw = hook_input.contains(&raw_marker);
    let hook_emitted_compact = hook_output.contains(&compact_marker);
    let model_saw_compact = last_message.contains(&compact_marker);
    let model_saw_raw = last_message.contains(&raw_marker);

    let verified = output.status.success()
        && hook_saw_raw
        && hook_emitted_compact
        && model_saw_compact
        && !model_saw_raw;
    let reason = if verified {
        "Codex PostToolUse hook replaced model-visible Bash output with compact feedback"
            .to_string()
    } else {
        format!(
            "HookReplacementFailed: status_success={} hook_saw_raw={} hook_emitted_compact={} model_saw_compact={} model_saw_raw={} stderr_summary={}",
            output.status.success(),
            hook_saw_raw,
            hook_emitted_compact,
            model_saw_compact,
            model_saw_raw,
            stderr_summary(&stderr)
        )
    };

    Ok(CanaryResult {
        verified,
        reason,
        canary_id,
        codex_version,
        raw_marker,
        compact_marker,
        workspace_path: workspace.display().to_string(),
        events_path: events_path.display().to_string(),
        stderr_path: stderr_path.display().to_string(),
        last_message_path: last_message_path.display().to_string(),
        hook_input_path: hook_input_path.display().to_string(),
        hook_output_path: hook_output_path.display().to_string(),
    })
}

pub fn load_latest_verified(
    path: &Path,
    current_version: Option<&str>,
) -> Result<Option<CanaryResult>> {
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    let result: CanaryResult = serde_json::from_str(&content)
        .with_context(|| format!("could not parse {}", path.display()))?;
    if !result.verified {
        return Ok(None);
    }
    if let (Some(expected), Some(actual)) = (current_version, result.codex_version.as_deref())
        && expected != actual
    {
        return Ok(None);
    }
    Ok(Some(result))
}

pub fn write_latest_verified(path: &Path, result: &CanaryResult) -> Result<()> {
    if !result.verified {
        bail!("cannot persist an unverified Codex output-replacement canary");
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_vec_pretty(result)?)
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(())
}

fn new_canary_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("canary-{}-{nanos}", std::process::id())
}

fn hook_config(hook_path: &Path) -> String {
    let command = hook_command(hook_path);
    format!(
        r#"[features]
hooks = true

[[hooks.PostToolUse]]
matcher = "*"

[[hooks.PostToolUse.hooks]]
type = "command"
command = {command:?}
timeout = 30
statusMessage = "Context Firewall canary"
"#
    )
}

fn hook_json(hook_path: &Path) -> String {
    serde_json::json!({
        "hooks": {
            "PostToolUse": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": hook_command(hook_path),
                    "timeout": 30,
                    "statusMessage": "Context Firewall canary"
                }]
            }]
        }
    })
    .to_string()
}

fn cli_hook_override(hook_path: &Path) -> String {
    let command = hook_command(hook_path);
    format!(
        "hooks.PostToolUse=[{{matcher=\"*\", hooks=[{{type=\"command\", command={command:?}, timeout=30, statusMessage=\"Context Firewall canary\"}}]}}]"
    )
}

fn hook_command(hook_path: &Path) -> String {
    format!(
        "/usr/bin/python3 {}",
        shell_quote(&hook_path.display().to_string())
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn stderr_summary(stderr: &str) -> String {
    let mut lines: Vec<&str> = stderr
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("hook")
                || lower.contains("error")
                || lower.contains("warn")
                || lower.contains("config")
        })
        .take(6)
        .collect();
    if lines.is_empty() {
        lines = stderr.lines().take(3).collect();
    }
    let summary = lines.join(" | ");
    if summary.chars().count() > 600 {
        summary.chars().take(600).collect::<String>() + "..."
    } else if summary.is_empty() {
        "none".to_string()
    } else {
        summary
    }
}

fn codex_home() -> Result<PathBuf> {
    if let Ok(home) = std::env::var("CODEX_HOME") {
        return Ok(PathBuf::from(home));
    }
    let home = std::env::var("HOME").context("HOME is not set; cannot locate CODEX_HOME")?;
    Ok(PathBuf::from(home).join(".codex"))
}

fn hook_script(
    raw_marker: &str,
    compact_marker: &str,
    hook_input_path: &Path,
    hook_output_path: &Path,
) -> String {
    format!(
        r#"import json
import sys

raw_marker = {raw_marker:?}
compact_marker = {compact_marker:?}
hook_input_path = {hook_input_path:?}
hook_output_path = {hook_output_path:?}

payload = json.load(sys.stdin)
serialized = json.dumps(payload, sort_keys=True)
open(hook_input_path, "w").write(json.dumps(payload, indent=2, sort_keys=True))

if raw_marker not in serialized:
    sys.exit(0)

replacement = compact_marker + " raw output stored locally; use cfw canary evidence for full hook input"
response = {{
    "decision": "block",
    "reason": replacement,
    "hookSpecificOutput": {{
        "hookEventName": "PostToolUse",
        "additionalContext": replacement
    }}
}}
open(hook_output_path, "w").write(json.dumps(response, indent=2, sort_keys=True))
print(json.dumps(response))
"#
    )
}

fn output_schema() -> &'static str {
    r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["visible_output", "raw_marker_seen", "compact_marker_seen"],
  "properties": {
    "visible_output": { "type": "string" },
    "raw_marker_seen": { "type": "boolean" },
    "compact_marker_seen": { "type": "boolean" }
  }
}
"#
}

fn read_optional(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("could not read {}", path.display())),
    }
}

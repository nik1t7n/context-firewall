use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    #[serde(default)]
    pub budgets: Budgets,
    #[serde(default)]
    pub actions: Actions,
    #[serde(default)]
    pub paths: PathRules,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Budgets {
    pub session_estimated_tokens: i64,
    pub turn_estimated_input_tokens: i64,
    pub tool_output_estimated_tokens: i64,
    pub artifact_retention_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Actions {
    #[serde(default = "default_compact_action")]
    pub large_test_output: PolicyAction,
    #[serde(default = "default_compact_action")]
    pub large_git_diff: PolicyAction,
    #[serde(default = "default_store_matches_action")]
    pub large_log: PolicyAction,
    #[serde(default = "default_block_action")]
    pub binary_file: PolicyAction,
    #[serde(default = "default_outline_action")]
    pub generated_file_read: PolicyAction,
    #[serde(default = "default_dedupe_action")]
    pub repeated_unchanged_output: PolicyAction,
    #[serde(default = "default_block_action")]
    pub node_modules_search: PolicyAction,
    #[serde(default = "default_store_matches_action")]
    pub large_search: PolicyAction,
    #[serde(default = "default_outline_action")]
    pub large_json: PolicyAction,
    #[serde(default = "default_store_matches_action")]
    pub large_listing: PolicyAction,
    #[serde(default = "default_compact_action")]
    pub browser_snapshot: PolicyAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathRules {
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub generated: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    Allow,
    Compact,
    StoreAndReturnHandle,
    StoreAndReturnMatches,
    Outline,
    Dedupe,
    Block,
    Ask,
}

impl PolicyAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Compact => "compact",
            Self::StoreAndReturnHandle => "store_and_return_handle",
            Self::StoreAndReturnMatches => "store_and_return_matches",
            Self::Outline => "outline",
            Self::Dedupe => "dedupe",
            Self::Block => "block",
            Self::Ask => "ask",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub action: PolicyAction,
    pub reason_code: &'static str,
    pub explanation: String,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            session_estimated_tokens: 200_000,
            turn_estimated_input_tokens: 30_000,
            tool_output_estimated_tokens: 6_000,
            artifact_retention_days: 14,
        }
    }
}

impl Default for Actions {
    fn default() -> Self {
        Self {
            large_test_output: PolicyAction::Compact,
            large_git_diff: PolicyAction::Compact,
            large_log: PolicyAction::StoreAndReturnMatches,
            binary_file: PolicyAction::Block,
            generated_file_read: PolicyAction::Outline,
            repeated_unchanged_output: PolicyAction::Dedupe,
            node_modules_search: PolicyAction::Block,
            large_search: PolicyAction::StoreAndReturnMatches,
            large_json: PolicyAction::Outline,
            large_listing: PolicyAction::StoreAndReturnMatches,
            browser_snapshot: PolicyAction::Compact,
        }
    }
}

impl Default for PathRules {
    fn default() -> Self {
        Self {
            deny: vec![
                "node_modules/**".to_string(),
                ".git/**".to_string(),
                "target/**".to_string(),
                "dist/**".to_string(),
                "build/**".to_string(),
            ],
            generated: vec![
                "**/*.generated.*".to_string(),
                "**/generated/**".to_string(),
                "**/*.lock".to_string(),
            ],
        }
    }
}

fn default_compact_action() -> PolicyAction {
    PolicyAction::Compact
}

fn default_store_matches_action() -> PolicyAction {
    PolicyAction::StoreAndReturnMatches
}

fn default_block_action() -> PolicyAction {
    PolicyAction::Block
}

fn default_outline_action() -> PolicyAction {
    PolicyAction::Outline
}

fn default_dedupe_action() -> PolicyAction {
    PolicyAction::Dedupe
}

impl Policy {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        toml::from_str(&content).with_context(|| format!("could not parse {}", path.display()))
    }

    pub fn write_default(path: &Path) -> Result<bool> {
        if path.exists() {
            return Ok(false);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        let content =
            toml::to_string_pretty(&Self::default()).context("could not encode default policy")?;
        std::fs::write(path, content)
            .with_context(|| format!("could not write {}", path.display()))?;
        Ok(true)
    }

    pub fn decide_command(&self, argv: &[String]) -> PolicyDecision {
        if argv.is_empty() {
            return PolicyDecision {
                action: PolicyAction::Block,
                reason_code: "empty_command",
                explanation: "empty commands cannot be executed".to_string(),
            };
        }

        let joined = argv.join(" ");
        let lower_joined = joined.to_ascii_lowercase();
        let program = argv[0].as_str();

        if matches!(
            program,
            "rg" | "grep"
                | "ag"
                | "ack"
                | "find"
                | "tree"
                | "ls"
                | "cat"
                | "tail"
                | "less"
                | "head"
        ) && argv.iter().skip(1).any(|arg| is_denied_path(arg))
        {
            return PolicyDecision {
                action: self.actions.node_modules_search,
                reason_code: "denied_path",
                explanation: "command targets dependency, VCS, build, or artifact paths"
                    .to_string(),
            };
        }

        if argv.iter().any(|arg| looks_binary_path(arg)) {
            return PolicyDecision {
                action: self.actions.binary_file,
                reason_code: "binary_file",
                explanation: "binary outputs are not useful as model-visible context".to_string(),
            };
        }

        if program == "git" && argv.iter().any(|arg| arg == "diff") {
            return PolicyDecision {
                action: self.actions.large_git_diff,
                reason_code: "git_diff",
                explanation: "git diffs can be large; compact and preserve retrieval handle"
                    .to_string(),
            };
        }

        if matches!(program, "rg" | "grep" | "ag" | "ack") {
            return PolicyDecision {
                action: self.actions.large_search,
                reason_code: "search_output",
                explanation: "search results are grouped by file and capped per file".to_string(),
            };
        }

        if matches!(program, "find" | "tree" | "ls") {
            return PolicyDecision {
                action: self.actions.large_listing,
                reason_code: "listing_output",
                explanation: "large path listings are capped and stored locally".to_string(),
            };
        }

        if is_browser_snapshot_command(argv) {
            return PolicyDecision {
                action: self.actions.browser_snapshot,
                reason_code: "browser_snapshot",
                explanation:
                    "browser accessibility snapshots are summarized by roles and key nodes"
                        .to_string(),
            };
        }

        if is_test_command(argv) {
            return PolicyDecision {
                action: self.actions.large_test_output,
                reason_code: "test_output",
                explanation: "test output is reduced to failures, summaries, and retrieval handles"
                    .to_string(),
            };
        }

        if matches!(program, "cat" | "tail" | "less" | "head")
            && argv.iter().any(|arg| arg.ends_with(".log"))
        {
            return PolicyDecision {
                action: self.actions.large_log,
                reason_code: "large_log",
                explanation: "log output is stored and reduced to relevant matches".to_string(),
            };
        }

        if matches!(program, "cat" | "tail" | "less" | "head")
            && argv.iter().any(|arg| looks_generated_path(arg))
        {
            return PolicyDecision {
                action: self.actions.generated_file_read,
                reason_code: "generated_file_read",
                explanation: "generated or lock files are outlined instead of pasted in full"
                    .to_string(),
            };
        }

        if (matches!(program, "cat" | "tail" | "less" | "head")
            && argv
                .iter()
                .any(|arg| arg.ends_with(".json") || arg.ends_with(".jsonl")))
            || matches!(program, "jq")
            || lower_joined.contains(" --json")
        {
            return PolicyDecision {
                action: self.actions.large_json,
                reason_code: "json_output",
                explanation: "JSON output is reduced to structure, counts, and scalar samples"
                    .to_string(),
            };
        }

        PolicyDecision {
            action: PolicyAction::Compact,
            reason_code: "default_compact",
            explanation: "default wrapper behavior stores raw output and returns compact output"
                .to_string(),
        }
    }
}

fn is_denied_path(value: &str) -> bool {
    let normalized = value.replace('\\', "/");
    path_has_component(&normalized, "node_modules")
        || path_has_component(&normalized, ".git")
        || (normalized.contains('/')
            && (path_has_component(&normalized, "target")
                || path_has_component(&normalized, "dist")
                || path_has_component(&normalized, "build")))
}

fn path_has_component(path: &str, component: &str) -> bool {
    path.split('/').any(|part| part == component)
}

fn looks_generated_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains(".generated.")
        || lower.contains("/generated/")
        || lower.ends_with(".lock")
        || lower.ends_with("package-lock.json")
        || lower.ends_with("pnpm-lock.yaml")
        || lower.ends_with("yarn.lock")
        || lower.ends_with("cargo.lock")
}

fn looks_binary_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".pdf", ".zip", ".tar", ".gz", ".tgz",
        ".xz", ".7z", ".dmg", ".sqlite", ".db", ".wasm", ".so", ".dylib", ".a", ".o", ".rlib",
        ".mp4", ".mov", ".mp3", ".wav",
    ]
    .iter()
    .any(|suffix| lower.ends_with(suffix))
}

fn is_test_command(argv: &[String]) -> bool {
    let joined = argv.join(" ");
    joined.contains("cargo test")
        || joined.contains("npm test")
        || joined.contains("pnpm test")
        || joined.contains("yarn test")
        || joined.contains("pytest")
        || joined.contains("go test")
        || joined.contains("vitest")
        || joined.contains("jest")
}

fn is_browser_snapshot_command(argv: &[String]) -> bool {
    let joined = argv.join(" ").to_ascii_lowercase();
    joined.contains("aria snapshot")
        || joined.contains("ariasnapshot")
        || joined.contains("accessibility snapshot")
        || joined.contains("accessibility.snapshot")
        || joined.contains("browser snapshot")
        || joined.contains("playwright")
            && (joined.contains("aria") || joined.contains("snapshot") || joined.contains("--ui"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_round_trips_as_toml() {
        let policy = Policy::default();
        let encoded = toml::to_string_pretty(&policy).expect("encode");
        let decoded: Policy = toml::from_str(&encoded).expect("decode");
        assert_eq!(policy, decoded);
    }

    #[test]
    fn git_diff_compacts() {
        let policy = Policy::default();
        let decision = policy.decide_command(&["git".to_string(), "diff".to_string()]);
        assert_eq!(decision.action, PolicyAction::Compact);
        assert_eq!(decision.reason_code, "git_diff");
    }

    #[test]
    fn node_modules_search_blocks() {
        let policy = Policy::default();
        let decision = policy.decide_command(&[
            "rg".to_string(),
            "needle".to_string(),
            "node_modules".to_string(),
        ]);
        assert_eq!(decision.action, PolicyAction::Block);
        assert_eq!(decision.reason_code, "denied_path");
    }

    #[test]
    fn search_commands_use_search_reducer_policy() {
        let policy = Policy::default();
        let decision = policy.decide_command(&["rg".to_string(), "needle".to_string()]);
        assert_eq!(decision.action, PolicyAction::StoreAndReturnMatches);
        assert_eq!(decision.reason_code, "search_output");
    }

    #[test]
    fn generated_reads_use_outline_policy() {
        let policy = Policy::default();
        let decision = policy.decide_command(&["cat".to_string(), "Cargo.lock".to_string()]);
        assert_eq!(decision.action, PolicyAction::Outline);
        assert_eq!(decision.reason_code, "generated_file_read");
    }

    #[test]
    fn browser_snapshot_commands_use_browser_snapshot_policy() {
        let policy = Policy::default();
        let decision = policy.decide_command(&[
            "node".to_string(),
            "-e".to_string(),
            "console.log('playwright aria snapshot')".to_string(),
        ]);
        assert_eq!(decision.action, PolicyAction::Compact);
        assert_eq!(decision.reason_code, "browser_snapshot");
    }

    #[test]
    fn binary_reads_are_blocked() {
        let policy = Policy::default();
        let decision = policy.decide_command(&["cat".to_string(), "image.png".to_string()]);
        assert_eq!(decision.action, PolicyAction::Block);
        assert_eq!(decision.reason_code, "binary_file");
    }
}

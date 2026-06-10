use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    pub budgets: Budgets,
    pub actions: Actions,
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
    pub large_test_output: PolicyAction,
    pub large_git_diff: PolicyAction,
    pub large_log: PolicyAction,
    pub binary_file: PolicyAction,
    pub generated_file_read: PolicyAction,
    pub repeated_unchanged_output: PolicyAction,
    pub node_modules_search: PolicyAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathRules {
    pub deny: Vec<String>,
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

impl Default for Policy {
    fn default() -> Self {
        Self {
            budgets: Budgets {
                session_estimated_tokens: 200_000,
                turn_estimated_input_tokens: 30_000,
                tool_output_estimated_tokens: 6_000,
                artifact_retention_days: 14,
            },
            actions: Actions {
                large_test_output: PolicyAction::Compact,
                large_git_diff: PolicyAction::Compact,
                large_log: PolicyAction::StoreAndReturnMatches,
                binary_file: PolicyAction::Block,
                generated_file_read: PolicyAction::Outline,
                repeated_unchanged_output: PolicyAction::Dedupe,
                node_modules_search: PolicyAction::Block,
            },
            paths: PathRules {
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
            },
        }
    }
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
        let program = argv[0].as_str();

        if joined.contains("node_modules")
            && matches!(program, "rg" | "grep" | "find" | "tree" | "ls")
        {
            return PolicyDecision {
                action: self.actions.node_modules_search,
                reason_code: "node_modules_search",
                explanation: "searching dependency directories is usually context waste"
                    .to_string(),
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

        if is_test_command(argv) {
            return PolicyDecision {
                action: self.actions.large_test_output,
                reason_code: "test_output",
                explanation: "test output is reduced to failures, summaries, and retrieval handles"
                    .to_string(),
            };
        }

        if matches!(program, "cat" | "tail" | "less") && joined.ends_with(".log") {
            return PolicyDecision {
                action: self.actions.large_log,
                reason_code: "large_log",
                explanation: "log output is stored and reduced to relevant matches".to_string(),
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
        assert_eq!(decision.reason_code, "node_modules_search");
    }
}

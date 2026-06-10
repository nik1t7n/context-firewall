use std::process::{Command, Output};

use cfw_reducers::reduce;
use tempfile::TempDir;

#[test]
fn real_cargo_test_failure_preserves_failure_signal() {
    let project = TempDir::new().expect("project temp dir");
    let target = TempDir::new().expect("target temp dir");
    std::fs::create_dir_all(project.path().join("src")).expect("src dir");
    std::fs::write(
        project.path().join("Cargo.toml"),
        r#"[package]
name = "cfw-real-corpus-failing-test"
version = "0.0.0"
edition = "2024"
"#,
    )
    .expect("cargo toml");
    std::fs::write(
        project.path().join("src/lib.rs"),
        r#"#[cfg(test)]
mod tests {
    #[test]
    fn preserves_real_failure_signal() {
        assert_eq!(1, 2, "context firewall corpus failure");
    }
}
"#,
    )
    .expect("lib rs");

    let output = Command::new("cargo")
        .args(["test", "--quiet", "--", "--test-threads", "1"])
        .env("CARGO_TARGET_DIR", target.path())
        .current_dir(project.path())
        .output()
        .expect("cargo test output");
    assert!(!output.status.success());

    let reduction = reduce("test-output", &combined_output(&output));

    assert_eq!(reduction.reducer, "test-output");
    assert!(reduction.output.contains("preserves_real_failure_signal"));
    assert!(reduction.output.contains("context firewall corpus failure"));
    assert!(reduction.output.contains("test result: FAILED"));
}

#[test]
fn real_git_diff_preserves_file_and_hunk_signal() {
    let work = TempDir::new().expect("work temp dir");
    let old = work.path().join("old.txt");
    let new = work.path().join("new.txt");
    std::fs::write(&old, "alpha\nold\nomega\n").expect("old");
    std::fs::write(&new, "alpha\nnew\nomega\n").expect("new");

    let output = Command::new("git")
        .args([
            "diff",
            "--no-index",
            old.to_str().expect("old path"),
            new.to_str().expect("new path"),
        ])
        .output()
        .expect("git diff output");
    assert!(!output.status.success());

    let reduction = reduce("git", &combined_output(&output));

    assert_eq!(reduction.reducer, "git");
    assert!(reduction.output.contains("diff --git"));
    assert!(reduction.output.contains("@@"));
    assert!(reduction.output.contains("-old"));
    assert!(reduction.output.contains("+new"));
}

#[test]
fn real_grep_output_groups_matches_by_file() {
    let work = TempDir::new().expect("work temp dir");
    let haystack = work.path().join("haystack.txt");
    let content = (1..=180)
        .map(|idx| format!("needle corpus line {idx}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&haystack, content).expect("haystack");

    let output = Command::new("grep")
        .args(["-Hn", "needle", haystack.to_str().expect("haystack path")])
        .output()
        .expect("grep output");
    assert!(output.status.success());

    let reduction = reduce("search", &combined_output(&output));

    assert_eq!(reduction.reducer, "search");
    assert!(reduction.omitted);
    assert!(
        reduction
            .output
            .contains("[context-firewall: search summary]")
    );
    assert!(reduction.output.contains("files matched: 1"));
    assert!(reduction.output.contains("raw match lines: 180"));
}

#[test]
fn real_jq_json_output_returns_shape_not_payload_blob() {
    let work = TempDir::new().expect("work temp dir");
    let payload = work.path().join("payload.json");
    let large_body = "x".repeat(400);
    std::fs::write(
        &payload,
        serde_json::json!({
            "ok": true,
            "items": (0..25).map(|idx| serde_json::json!({
                "id": idx,
                "body": large_body,
            })).collect::<Vec<_>>()
        })
        .to_string(),
    )
    .expect("payload");

    let output = Command::new("jq")
        .args([".", payload.to_str().expect("payload path")])
        .output()
        .expect("jq output");
    assert!(output.status.success());

    let reduction = reduce("json", &combined_output(&output));

    assert_eq!(reduction.reducer, "json");
    assert!(reduction.output.contains("$.items: array(len=25)"));
    assert!(reduction.output.contains("$.ok: bool(true)"));
    assert!(!reduction.output.contains(&"x".repeat(120)));
}

fn combined_output(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
        stdout.to_string()
    } else if stdout.trim().is_empty() {
        stderr.to_string()
    } else {
        format!("{stdout}\n[stderr]\n{stderr}")
    }
}

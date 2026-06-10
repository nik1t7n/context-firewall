use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn run_receipt_and_show_use_real_artifacts() {
    let temp = TempDir::new().expect("temp dir");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    let output = run
        .env("CFW_DATA_DIR", temp.path())
        .env("CFW_SESSION", "test-session")
        .args(["run", "--", "printf", "alpha\\nbeta\\n"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[context-firewall]"))
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("utf8 stdout");
    let span_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("span: cfw://span/"))
        .expect("span id")
        .to_string();

    let mut receipt = Command::cargo_bin("cfw").expect("cfw binary");
    receipt
        .env("CFW_DATA_DIR", temp.path())
        .arg("receipt")
        .assert()
        .success()
        .stdout(predicate::str::contains("Context Firewall Receipt"))
        .stdout(predicate::str::contains("advisory_wrapper"));

    let mut json_receipt = Command::cargo_bin("cfw").expect("cfw binary");
    json_receipt
        .env("CFW_DATA_DIR", temp.path())
        .args(["receipt", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"net_estimated_saved\""))
        .stdout(predicate::str::contains("\"advisory_wrapper\""));

    let mut top = Command::cargo_bin("cfw").expect("cfw binary");
    top.env("CFW_DATA_DIR", temp.path())
        .arg("top")
        .assert()
        .success()
        .stdout(predicate::str::contains("Context Firewall Top Burners"))
        .stdout(predicate::str::contains("printf"));

    let mut show = Command::cargo_bin("cfw").expect("cfw binary");
    show.env("CFW_DATA_DIR", temp.path())
        .args(["show", &span_id, "--lines", "1:1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1: alpha"));
}

#[test]
fn doctor_reports_codex_without_claiming_hook_replacement() {
    let temp = TempDir::new().expect("temp dir");

    let mut doctor = Command::cargo_bin("cfw").expect("cfw binary");
    doctor
        .env("CFW_DATA_DIR", temp.path())
        .args(["doctor", "codex"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hook_replacement_verified: false"));
}

#[test]
fn first_run_creates_a_real_span() {
    let temp = TempDir::new().expect("temp dir");

    let mut first_run = Command::cargo_bin("cfw").expect("cfw binary");
    first_run
        .env("CFW_DATA_DIR", temp.path())
        .env("CFW_SESSION", "first-run-session")
        .arg("first-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("context_firewall_demo"))
        .stdout(predicate::str::contains("[context-firewall]"));

    let mut receipt = Command::cargo_bin("cfw").expect("cfw binary");
    receipt
        .env("CFW_DATA_DIR", temp.path())
        .arg("receipt")
        .assert()
        .success()
        .stdout(predicate::str::contains("spans: 1"));
}

#[test]
fn install_codex_wrapper_prints_advisory_snippet() {
    let mut install = Command::cargo_bin("cfw").expect("cfw binary");
    install
        .args(["install", "codex", "--mode", "wrapper"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mode: wrapper"))
        .stdout(predicate::str::contains("enforcement: advisory"))
        .stdout(predicate::str::contains("context-firewall:start"));
}

#[test]
fn install_codex_wrapper_writes_agents_block_idempotently() {
    let temp = TempDir::new().expect("temp dir");
    let agents = temp.path().join("AGENTS.md");

    let mut first = Command::cargo_bin("cfw").expect("cfw binary");
    first
        .current_dir(temp.path())
        .args([
            "install",
            "codex",
            "--mode",
            "wrapper",
            "--write-agents",
            "--agents-path",
            agents.to_str().expect("utf8 path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("result: Written"));

    let mut second = Command::cargo_bin("cfw").expect("cfw binary");
    second
        .current_dir(temp.path())
        .args([
            "install",
            "codex",
            "--mode",
            "wrapper",
            "--write-agents",
            "--agents-path",
            agents.to_str().expect("utf8 path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("result: AlreadyPresent"));

    let content = std::fs::read_to_string(agents).expect("agents content");
    assert_eq!(content.matches("context-firewall:start").count(), 1);
}

#[test]
fn install_codex_hook_native_is_explicitly_blocked() {
    let mut install = Command::cargo_bin("cfw").expect("cfw binary");
    install
        .args(["install", "codex", "--mode", "hook-native"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("HookReplacementFailed"));
}

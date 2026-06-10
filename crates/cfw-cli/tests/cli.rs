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

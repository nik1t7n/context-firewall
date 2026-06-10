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
    let artifact_dir = temp.path().join("sessions/test-session/artifacts");
    assert!(artifact_dir.join(format!("{span_id}.txt")).exists());
    assert!(artifact_dir.join(format!("{span_id}.stdout")).exists());
    assert!(artifact_dir.join(format!("{span_id}.stderr")).exists());
    let meta_path = artifact_dir.join(format!("{span_id}.meta.json"));
    assert!(meta_path.exists());
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(meta_path).expect("meta"))
            .expect("meta json");
    assert_eq!(meta["argv"][0], "printf");

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

    let mut spans = Command::cargo_bin("cfw").expect("cfw binary");
    spans
        .env("CFW_DATA_DIR", temp.path())
        .arg("spans")
        .assert()
        .success()
        .stdout(predicate::str::contains("Context Firewall Spans"))
        .stdout(predicate::str::contains("printf"));

    let mut spans_json = Command::cargo_bin("cfw").expect("cfw binary");
    spans_json
        .env("CFW_DATA_DIR", temp.path())
        .args(["spans", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"session_id\": \"test-session\""));

    let mut show = Command::cargo_bin("cfw").expect("cfw binary");
    show.env("CFW_DATA_DIR", temp.path())
        .args(["show", &span_id, "--lines", "1:1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1: alpha"));
}

#[test]
fn show_guards_secret_like_raw_output_unless_forced() {
    let temp = TempDir::new().expect("temp dir");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    let output = run
        .env("CFW_DATA_DIR", temp.path())
        .env("CFW_SESSION", "secret-session")
        .args([
            "run",
            "--",
            "printf",
            "api_key=abcdefghijklmnopqrstuvwxyz123456",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("utf8 stdout");
    let span_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("span: cfw://span/"))
        .expect("span id")
        .to_string();

    let mut guarded_show = Command::cargo_bin("cfw").expect("cfw binary");
    guarded_show
        .env("CFW_DATA_DIR", temp.path())
        .args(["show", &span_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("SecretGuard"));

    let mut forced_show = Command::cargo_bin("cfw").expect("cfw binary");
    forced_show
        .env("CFW_DATA_DIR", temp.path())
        .args(["show", &span_id, "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("api_key="));
}

#[test]
fn purge_all_removes_span_rows_and_artifacts() {
    let temp = TempDir::new().expect("temp dir");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    let output = run
        .env("CFW_DATA_DIR", temp.path())
        .env("CFW_SESSION", "purge-session")
        .args(["run", "--", "printf", "alpha"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("utf8 stdout");
    let span_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("span: cfw://span/"))
        .expect("span id")
        .to_string();
    let artifact = temp
        .path()
        .join("sessions/purge-session/artifacts")
        .join(format!("{span_id}.txt"));
    assert!(artifact.exists());

    let mut purge = Command::cargo_bin("cfw").expect("cfw binary");
    purge
        .env("CFW_DATA_DIR", temp.path())
        .args(["purge", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("purged spans: 1"));

    assert!(!artifact.exists());

    let mut spans = Command::cargo_bin("cfw").expect("cfw binary");
    spans
        .env("CFW_DATA_DIR", temp.path())
        .arg("spans")
        .assert()
        .success()
        .stdout(predicate::str::contains("Context Firewall Spans"))
        .stdout(predicate::str::contains("printf").not());
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

#[test]
fn policy_init_check_and_explain_work() {
    let temp = TempDir::new().expect("temp dir");

    let mut init = Command::cargo_bin("cfw").expect("cfw binary");
    init.env("CFW_DATA_DIR", temp.path())
        .args(["policy", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("created policy"));

    let mut check = Command::cargo_bin("cfw").expect("cfw binary");
    check
        .env("CFW_DATA_DIR", temp.path())
        .args(["policy", "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("policy: ok"));

    let mut explain = Command::cargo_bin("cfw").expect("cfw binary");
    explain
        .env("CFW_DATA_DIR", temp.path())
        .args(["policy", "explain", "--", "git", "diff"])
        .assert()
        .success()
        .stdout(predicate::str::contains("action: compact"))
        .stdout(predicate::str::contains("reason: git_diff"));
}

#[test]
fn policy_blocks_obvious_dependency_search() {
    let temp = TempDir::new().expect("temp dir");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    run.env("CFW_DATA_DIR", temp.path())
        .args(["run", "--", "rg", "needle", "node_modules"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("PolicyBlocked"))
        .stderr(predicate::str::contains("denied_path"));
}

#[test]
fn git_diff_uses_git_reducer_by_policy() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    let old = work.path().join("old.txt");
    let new = work.path().join("new.txt");
    std::fs::write(&old, "old\nsame\n").expect("old file");
    std::fs::write(&new, "new\nsame\n").expect("new file");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    run.env("CFW_DATA_DIR", data.path())
        .args([
            "run",
            "--",
            "git",
            "diff",
            "--no-index",
            old.to_str().expect("utf8 old"),
            new.to_str().expect("utf8 new"),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("diff --git"))
        .stdout(predicate::str::contains(
            "delivery_status: advisory_wrapper",
        ));

    let mut receipt = Command::cargo_bin("cfw").expect("cfw binary");
    receipt
        .env("CFW_DATA_DIR", data.path())
        .arg("receipt")
        .assert()
        .success()
        .stdout(predicate::str::contains(" git "));
}

#[test]
fn search_output_uses_search_reducer_by_policy() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    let haystack = work.path().join("haystack.txt");
    let content = (1..=150)
        .map(|idx| format!("needle line {idx}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&haystack, content).expect("haystack");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    run.env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args(["run", "--", "grep", "-Hn", "needle", "haystack.txt"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "[context-firewall: search summary]",
        ))
        .stdout(predicate::str::contains("haystack.txt"));

    let mut explain = Command::cargo_bin("cfw").expect("cfw binary");
    explain
        .env("CFW_DATA_DIR", data.path())
        .args([
            "policy",
            "explain",
            "--",
            "grep",
            "-Hn",
            "needle",
            "haystack.txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("reason: search_output"));
}

#[test]
fn logs_json_and_generated_files_route_to_specialized_reducers() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");

    let log_path = work.path().join("app.log");
    let log_content = (1..=150)
        .map(|idx| {
            if idx == 80 {
                "ERROR connection refused".to_string()
            } else {
                format!("INFO heartbeat {idx}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&log_path, log_content).expect("log");

    let json_path = work.path().join("payload.json");
    std::fs::write(
        &json_path,
        serde_json::json!({"items": [1, 2, 3, 4], "ok": true}).to_string(),
    )
    .expect("json");

    let generated_path = work.path().join("client.generated.ts");
    std::fs::write(
        &generated_path,
        "import x from 'x';\n\nexport function generatedClient() {\n  return x;\n}\n",
    )
    .expect("generated");

    let mut log_run = Command::cargo_bin("cfw").expect("cfw binary");
    log_run
        .env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args(["run", "--", "cat", "app.log"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ERROR connection refused"))
        .stdout(predicate::str::contains("omitted lines"));

    let mut json_run = Command::cargo_bin("cfw").expect("cfw binary");
    json_run
        .env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args(["run", "--", "cat", "payload.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[context-firewall: json shape]"))
        .stdout(predicate::str::contains("$.items: array(len=4)"));

    let mut generated_run = Command::cargo_bin("cfw").expect("cfw binary");
    generated_run
        .env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args(["run", "--", "cat", "client.generated.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[context-firewall: file outline]"))
        .stdout(predicate::str::contains("generatedClient"));
}

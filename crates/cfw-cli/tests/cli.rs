use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn no_args_shows_launch_screen() {
    let mut cfw = Command::cargo_bin("cfw").expect("cfw binary");
    cfw.assert()
        .success()
        .stdout(predicate::str::contains("Context Firewall"))
        .stdout(predicate::str::contains("cfw first-run"))
        .stdout(predicate::str::contains("cfw install agent"));
}

#[test]
fn launch_screen_can_force_color() {
    let mut cfw = Command::cargo_bin("cfw").expect("cfw binary");
    cfw.env("CFW_COLOR", "always")
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{1b}[1;38;2;255;28;28m"))
        .stdout(predicate::str::contains("\u{1b}[1;38;2;255;111;10mStart"));
}

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
    let json_receipt_output = json_receipt
        .env("CFW_DATA_DIR", temp.path())
        .args(["receipt", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"net_estimated_saved\""))
        .stdout(predicate::str::contains("\"advisory_wrapper\""))
        .get_output()
        .stdout
        .clone();
    let receipt_json: serde_json::Value =
        serde_json::from_slice(&json_receipt_output).expect("receipt json");
    assert_eq!(receipt_json["schema_version"], "cfw.receipt.v1");

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

    let mut gain = Command::cargo_bin("cfw").expect("cfw binary");
    gain.env("CFW_DATA_DIR", temp.path())
        .arg("gain")
        .assert()
        .success()
        .stdout(predicate::str::contains("Context Firewall Gain"))
        .stdout(predicate::str::contains("saved estimated tokens"));

    let mut discover = Command::cargo_bin("cfw").expect("cfw binary");
    discover
        .env("CFW_DATA_DIR", temp.path())
        .arg("discover")
        .assert()
        .success()
        .stdout(predicate::str::contains("Context Firewall Discover"))
        .stdout(predicate::str::contains("largest raw outputs"))
        .stdout(predicate::str::contains("repeated passthrough"));

    let mut session = Command::cargo_bin("cfw").expect("cfw binary");
    session
        .env("CFW_DATA_DIR", temp.path())
        .arg("session")
        .assert()
        .success()
        .stdout(predicate::str::contains("Context Firewall Session"))
        .stdout(predicate::str::contains("cfw-routed commands: 1"));

    let mut show = Command::cargo_bin("cfw").expect("cfw binary");
    show.env("CFW_DATA_DIR", temp.path())
        .args(["show", &span_id, "--lines", "1:1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1: alpha"));

    let mut grep_show = Command::cargo_bin("cfw").expect("cfw binary");
    grep_show
        .env("CFW_DATA_DIR", temp.path())
        .args(["show", &span_id, "--grep", "beta", "--around", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1: alpha"))
        .stdout(predicate::str::contains("2: beta"));

    let mut search_spans = Command::cargo_bin("cfw").expect("cfw binary");
    search_spans
        .env("CFW_DATA_DIR", temp.path())
        .args(["search-spans", "beta"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&span_id))
        .stdout(predicate::str::contains("2: beta"));
}

#[test]
fn receipt_schema_matches_json_contract() {
    let temp = TempDir::new().expect("temp dir");

    let mut schema = Command::cargo_bin("cfw").expect("cfw binary");
    let schema_output = schema
        .env("CFW_DATA_DIR", temp.path())
        .args(["receipt", "--schema"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let schema_json: serde_json::Value =
        serde_json::from_slice(&schema_output).expect("schema json");
    assert_eq!(
        schema_json["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema_json["properties"]["schema_version"]["const"],
        "cfw.receipt.v1"
    );
    assert_eq!(
        schema_json["properties"]["recent_spans"]["items"]["properties"]["delivery_status"]["enum"]
            [1],
        "advisory_wrapper"
    );

    let mut receipt = Command::cargo_bin("cfw").expect("cfw binary");
    let receipt_output = receipt
        .env("CFW_DATA_DIR", temp.path())
        .args(["receipt", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let receipt_json: serde_json::Value =
        serde_json::from_slice(&receipt_output).expect("receipt json");
    assert_eq!(
        receipt_json["schema_version"],
        schema_json["properties"]["schema_version"]["const"]
    );
    assert!(receipt_json["recent_spans"].is_array());
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
fn repeated_identical_command_returns_duplicate_handle() {
    let temp = TempDir::new().expect("temp dir");
    let work = TempDir::new().expect("work dir");
    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(work.path())
        .status()
        .expect("git init");

    let mut first = Command::cargo_bin("cfw").expect("cfw binary");
    first
        .env("CFW_DATA_DIR", temp.path())
        .env("CFW_SESSION", "dedupe-session")
        .current_dir(work.path())
        .args(["run", "--", "seq", "1", "220"])
        .assert()
        .success()
        .stdout(predicate::str::contains("220"))
        .stdout(predicate::str::contains("duplicate output").not());

    let mut second = Command::cargo_bin("cfw").expect("cfw binary");
    second
        .env("CFW_DATA_DIR", temp.path())
        .env("CFW_SESSION", "dedupe-session")
        .current_dir(work.path())
        .args(["run", "--", "seq", "1", "220"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "[context-firewall: duplicate output]",
        ))
        .stdout(predicate::str::contains("previous_span: cfw://span/"))
        .stdout(predicate::str::contains("same repeat fingerprint"));

    let mut spans = Command::cargo_bin("cfw").expect("cfw binary");
    spans
        .env("CFW_DATA_DIR", temp.path())
        .args(["spans", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"risk_class\": \"deduped\""));
}

#[test]
fn changed_input_file_prevents_duplicate_handle_even_with_same_output() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    let watched = work.path().join("watched.txt");
    std::fs::write(&watched, "version one\n").expect("write first input");

    let watched_arg = watched.to_str().expect("utf8 watched path");
    let script =
        "BEGIN { while ((getline < ARGV[1]) > 0) {} for (i = 1; i <= 220; i++) print i; exit }";

    let mut first = Command::cargo_bin("cfw").expect("cfw binary");
    first
        .env("CFW_DATA_DIR", data.path())
        .env("CFW_SESSION", "input-fingerprint-session")
        .current_dir(work.path())
        .args(["run", "--", "awk", script, watched_arg])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate output").not());

    std::fs::write(&watched, "version two\n").expect("write second input");

    let mut second = Command::cargo_bin("cfw").expect("cfw binary");
    second
        .env("CFW_DATA_DIR", data.path())
        .env("CFW_SESSION", "input-fingerprint-session")
        .current_dir(work.path())
        .args(["run", "--", "awk", script, watched_arg])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate output").not());

    let mut spans = Command::cargo_bin("cfw").expect("cfw binary");
    spans
        .env("CFW_DATA_DIR", data.path())
        .args(["spans", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"repeat_key\":"))
        .stdout(predicate::str::contains("\"repeat_evidence_json\":"))
        .stdout(predicate::str::contains("\"risk_class\": \"deduped\"").not());
}

#[test]
fn changed_stdin_file_prevents_duplicate_handle_even_with_same_output() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    let stdin_file = work.path().join("stdin.txt");
    std::fs::write(&stdin_file, "version one\n").expect("write first stdin");

    let stdin_arg = stdin_file.to_str().expect("utf8 stdin path");
    let stdin_canonical = stdin_file
        .canonicalize()
        .expect("canonical stdin path")
        .display()
        .to_string();
    let script =
        "while read line; do :; done; i=1; while [ $i -le 220 ]; do echo $i; i=$((i + 1)); done";

    let mut first = Command::cargo_bin("cfw").expect("cfw binary");
    first
        .env("CFW_DATA_DIR", data.path())
        .env("CFW_SESSION", "stdin-fingerprint-session")
        .current_dir(work.path())
        .args(["run", "--stdin-file", stdin_arg, "--", "sh", "-c", script])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate output").not());

    std::fs::write(&stdin_file, "version two\n").expect("write second stdin");

    let mut second = Command::cargo_bin("cfw").expect("cfw binary");
    second
        .env("CFW_DATA_DIR", data.path())
        .env("CFW_SESSION", "stdin-fingerprint-session")
        .current_dir(work.path())
        .args(["run", "--stdin-file", stdin_arg, "--", "sh", "-c", script])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate output").not());

    let mut spans = Command::cargo_bin("cfw").expect("cfw binary");
    let spans_output = spans
        .env("CFW_DATA_DIR", data.path())
        .args(["spans", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"risk_class\": \"deduped\"").not())
        .get_output()
        .stdout
        .clone();
    let spans_json: serde_json::Value = serde_json::from_slice(&spans_output).expect("spans json");
    let spans = spans_json.as_array().expect("spans array");
    assert_eq!(spans.len(), 2);
    let mut stdin_hashes = spans
        .iter()
        .map(|span| {
            let evidence: serde_json::Value = serde_json::from_str(
                span["repeat_evidence_json"]
                    .as_str()
                    .expect("evidence json"),
            )
            .expect("repeat evidence");
            assert_eq!(evidence["stdin"]["source"], "file");
            assert_eq!(evidence["stdin"]["path"], stdin_canonical);
            evidence["stdin"]["hash"]
                .as_str()
                .expect("stdin hash")
                .to_string()
        })
        .collect::<Vec<_>>();
    stdin_hashes.sort();
    stdin_hashes.dedup();
    assert_eq!(stdin_hashes.len(), 2);
}

#[test]
fn changed_cargo_lock_prevents_duplicate_handle_even_with_same_output() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    std::fs::write(
        work.path().join("Cargo.toml"),
        "[package]\nname = \"fingerprint-smoke\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        work.path().join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n",
    )
    .expect("write first Cargo.lock");

    let mut first = Command::cargo_bin("cfw").expect("cfw binary");
    first
        .env("CFW_DATA_DIR", data.path())
        .env("CFW_SESSION", "cargo-dependency-fingerprint-session")
        .current_dir(work.path())
        .args(["run", "--", "cargo", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate output").not());

    let mut duplicate = Command::cargo_bin("cfw").expect("cfw binary");
    duplicate
        .env("CFW_DATA_DIR", data.path())
        .env("CFW_SESSION", "cargo-dependency-fingerprint-session")
        .current_dir(work.path())
        .args(["run", "--", "cargo", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "[context-firewall: duplicate output]",
        ));

    std::fs::write(
        work.path().join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"changed\"\nversion = \"1.0.0\"\n",
    )
    .expect("write changed Cargo.lock");

    let mut changed = Command::cargo_bin("cfw").expect("cfw binary");
    changed
        .env("CFW_DATA_DIR", data.path())
        .env("CFW_SESSION", "cargo-dependency-fingerprint-session")
        .current_dir(work.path())
        .args(["run", "--", "cargo", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate output").not());

    let mut spans = Command::cargo_bin("cfw").expect("cfw binary");
    let spans_output = spans
        .env("CFW_DATA_DIR", data.path())
        .args(["spans", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let spans_json: serde_json::Value = serde_json::from_slice(&spans_output).expect("spans json");
    let spans = spans_json.as_array().expect("spans array");
    let mut lock_hashes = spans
        .iter()
        .filter_map(|span| {
            let evidence: serde_json::Value = serde_json::from_str(
                span["repeat_evidence_json"]
                    .as_str()
                    .expect("evidence json"),
            )
            .expect("repeat evidence");
            evidence["dependencies"]
                .as_array()?
                .iter()
                .flat_map(|dependency| dependency["files"].as_array().into_iter().flatten())
                .find(|file| {
                    file["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("Cargo.lock"))
                })
                .and_then(|file| file["hash"].as_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    lock_hashes.sort();
    lock_hashes.dedup();
    assert_eq!(lock_hashes.len(), 2);
}

#[test]
fn repeated_tiny_output_is_not_deduped_when_receipt_would_be_larger() {
    let temp = TempDir::new().expect("temp dir");

    for _ in 0..2 {
        let mut run = Command::cargo_bin("cfw").expect("cfw binary");
        run.env("CFW_DATA_DIR", temp.path())
            .env("CFW_SESSION", "tiny-dedupe-session")
            .args(["run", "--", "printf", "x"])
            .assert()
            .success()
            .stdout(predicate::str::contains("x"))
            .stdout(predicate::str::contains("duplicate output").not());
    }

    let mut spans = Command::cargo_bin("cfw").expect("cfw binary");
    spans
        .env("CFW_DATA_DIR", temp.path())
        .args(["spans", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"risk_class\": \"deduped\"").not());
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
        .stdout(predicate::str::contains("target: codex"))
        .stdout(predicate::str::contains("mcp: cfw mcp"))
        .stdout(predicate::str::contains("context-firewall:start"));
}

#[test]
fn mcp_lists_context_firewall_tools() {
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("cfw"))
        .arg("mcp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn cfw mcp");

    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(
            stdin,
            "{}",
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})
        )
        .expect("write initialize");
        writeln!(
            stdin,
            "{}",
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})
        )
        .expect("write tools/list");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("mcp output");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let messages = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json rpc"))
        .collect::<Vec<_>>();
    assert_eq!(
        messages[0]["result"]["serverInfo"]["name"],
        "context-firewall"
    );
    let tools = messages[1]["result"]["tools"].as_array().expect("tools");
    assert!(tools.iter().any(|tool| tool["name"] == "cfw_run"));
    assert!(tools.iter().any(|tool| tool["name"] == "cfw_show"));
    assert!(tools.iter().any(|tool| tool["name"] == "cfw_receipt"));
}

#[test]
fn install_agent_configs_for_gemini_claude_cursor_and_antigravity() {
    let temp = TempDir::new().expect("temp dir");
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home).expect("home");

    for target in ["gemini", "claude", "cursor", "antigravity"] {
        let mut install = Command::cargo_bin("cfw").expect("cfw binary");
        install
            .current_dir(temp.path())
            .env("HOME", &home)
            .args(["install", target])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!("target: {target}")));
    }

    let gemini: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".gemini/settings.json"))
            .expect("gemini settings"),
    )
    .expect("gemini json");
    assert_eq!(gemini["mcpServers"]["context-firewall"]["command"], "cfw");
    assert_eq!(gemini["mcpServers"]["context-firewall"]["args"][0], "mcp");
    let gemini_user: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".gemini/settings.json")).expect("gemini user settings"),
    )
    .expect("gemini user json");
    assert_eq!(
        gemini_user["mcpServers"]["context-firewall"]["command"],
        "cfw"
    );

    let claude: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".mcp.json")).expect("claude mcp"),
    )
    .expect("claude json");
    assert_eq!(claude["mcpServers"]["context-firewall"]["type"], "stdio");
    assert!(
        std::fs::read_to_string(temp.path().join("CLAUDE.md"))
            .expect("claude md")
            .contains("@AGENTS.md")
    );

    let cursor: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".cursor/mcp.json")).expect("cursor mcp"),
    )
    .expect("cursor json");
    assert_eq!(cursor["mcpServers"]["context-firewall"]["command"], "cfw");
    assert!(
        temp.path()
            .join(".cursor/rules/context-firewall.mdc")
            .exists()
    );

    let agy: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(".gemini/antigravity-cli/mcp_config.json"))
            .expect("agy cli mcp"),
    )
    .expect("agy json");
    assert_eq!(agy["mcpServers"]["context-firewall"]["command"], "cfw");
    assert!(temp.path().join(".antigravity/mcp_config.json").exists());
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
fn install_codex_wrapper_dry_run_does_not_write_agents_file() {
    let temp = TempDir::new().expect("temp dir");
    let agents = temp.path().join("AGENTS.md");

    let mut install = Command::cargo_bin("cfw").expect("cfw binary");
    install
        .current_dir(temp.path())
        .args([
            "install",
            "codex",
            "--mode",
            "wrapper",
            "--write-agents",
            "--dry-run",
            "--agents-path",
            agents.to_str().expect("utf8 path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry_run: true"))
        .stdout(predicate::str::contains("result: Written"))
        .stdout(predicate::str::contains("context-firewall:start"));

    assert!(!agents.exists());
}

#[test]
fn uninstall_codex_wrapper_removes_only_managed_agents_block() {
    let temp = TempDir::new().expect("temp dir");
    let agents = temp.path().join("AGENTS.md");
    std::fs::write(&agents, "# Project Rules\n\nKeep this line.\n").expect("agents seed");

    let mut install = Command::cargo_bin("cfw").expect("cfw binary");
    install
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

    let mut uninstall = Command::cargo_bin("cfw").expect("cfw binary");
    uninstall
        .current_dir(temp.path())
        .args([
            "uninstall",
            "codex",
            "--agents-path",
            agents.to_str().expect("utf8 path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("result: Removed"));

    let content = std::fs::read_to_string(&agents).expect("agents content");
    assert!(content.contains("Keep this line."));
    assert!(!content.contains("context-firewall:start"));

    let mut second = Command::cargo_bin("cfw").expect("cfw binary");
    second
        .current_dir(temp.path())
        .args([
            "uninstall",
            "codex",
            "--agents-path",
            agents.to_str().expect("utf8 path"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("result: AlreadyAbsent"));
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
#[cfg(unix)]
fn policy_blocks_symlink_to_denied_path() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    let package_dir = work.path().join("node_modules/pkg");
    std::fs::create_dir_all(&package_dir).expect("package dir");
    std::fs::write(package_dir.join("index.js"), "console.log('burn');\n").expect("package file");
    std::os::unix::fs::symlink(&package_dir, work.path().join("vendor")).expect("symlink");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    run.env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args(["run", "--", "cat", "vendor/index.js"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("PolicyBlocked"))
        .stderr(predicate::str::contains("denied_path"));
}

#[test]
fn policy_blocks_case_variant_denied_path() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    let package_dir = work.path().join("Node_Modules/pkg");
    std::fs::create_dir_all(&package_dir).expect("package dir");
    std::fs::write(package_dir.join("index.js"), "console.log('burn');\n").expect("package file");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    run.env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args(["run", "--", "cat", "Node_Modules/pkg/index.js"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("PolicyBlocked"))
        .stderr(predicate::str::contains("denied_path"));
}

#[test]
fn policy_ask_fails_noninteractive_without_running_command() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    std::fs::write(
        data.path().join("config.toml"),
        "[actions]\nlarge_listing = \"ask\"\n",
    )
    .expect("policy config");

    let mut explain = Command::cargo_bin("cfw").expect("cfw binary");
    explain
        .env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args(["policy", "explain", "--", "find", ".", "-maxdepth", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("action: ask"))
        .stdout(predicate::str::contains("reason: listing_output"));

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    run.env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args(["run", "--", "find", ".", "-maxdepth", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("PolicyAskRequired"))
        .stderr(predicate::str::contains("command was not executed"));

    let artifacts_root = data.path().join("sessions");
    assert!(!artifacts_root.exists());
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
fn browser_snapshot_output_uses_browser_reducer_by_policy() {
    let data = TempDir::new().expect("data dir");
    let work = TempDir::new().expect("work dir");
    let snapshot = work.path().join("aria-snapshot.txt");
    let mut lines = vec![
        "url: https://example.test/app".to_string(),
        "title: Example App".to_string(),
        "- banner:".to_string(),
        "  - heading \"Example App\" [level=1]".to_string(),
        "  - link \"Docs\"".to_string(),
        "- main:".to_string(),
        "  - textbox \"Search\"".to_string(),
        "  - button \"Create\"".to_string(),
    ];
    for idx in 1..=180 {
        lines.push(format!("  - text \"Noise row {idx}\""));
    }
    lines.push("  - alert \"Save failed\"".to_string());
    std::fs::write(&snapshot, lines.join("\n")).expect("snapshot");

    let mut run = Command::cargo_bin("cfw").expect("cfw binary");
    run.env("CFW_DATA_DIR", data.path())
        .current_dir(work.path())
        .args([
            "run",
            "--",
            "node",
            "-e",
            "const fs=require('fs'); console.log('playwright aria snapshot'); console.log(fs.readFileSync('aria-snapshot.txt','utf8'))",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "[context-firewall: browser snapshot summary]",
        ))
        .stdout(predicate::str::contains("key accessible nodes"))
        .stdout(predicate::str::contains("button \"Create\""))
        .stdout(predicate::str::contains("alert \"Save failed\""))
        .stdout(predicate::str::contains("Noise row 90").not())
        .stdout(predicate::str::contains(
            "delivery_status: advisory_wrapper",
        ));

    let mut spans = Command::cargo_bin("cfw").expect("cfw binary");
    spans
        .env("CFW_DATA_DIR", data.path())
        .args(["spans", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"kind\": \"browser-snapshot\""));
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

use cfw_reducers::reduce;

#[test]
fn generic_reducer_keeps_small_output_intact() {
    let input = "one\ntwo\nthree\n";
    let reduction = reduce("generic", input);

    assert_eq!(reduction.reducer, "generic");
    assert_eq!(reduction.output, input);
    assert!(!reduction.omitted);
}

#[test]
fn generic_reducer_marks_omitted_middle_lines() {
    let input = (1..=150)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    let reduction = reduce("generic", &input);

    assert!(reduction.omitted);
    assert!(reduction.output.contains("line 1"));
    assert!(reduction.output.contains("line 150"));
    assert!(reduction.output.contains("omitted 30 middle lines"));
}

#[test]
fn test_output_reducer_preserves_failure_signal() {
    let input = [
        "running 3 tests",
        "test passing_one ... ok",
        "test failing_one ... FAILED",
        "thread 'failing_one' panicked at src/lib.rs:7:5:",
        "assertion failed: left == right",
        "failures:",
        "    failing_one",
        "test result: FAILED. 2 passed; 1 failed",
    ]
    .join("\n");

    let reduction = reduce("test-output", &input);

    assert_eq!(reduction.reducer, "test-output");
    assert!(reduction.output.contains("failing_one"));
    assert!(reduction.output.contains("panicked"));
    assert!(reduction.output.contains("test result: FAILED"));
}

#[test]
fn test_output_reducer_preserves_tool_diagnostics() {
    let input = (1..=80)
        .map(|line| {
            if line == 35 {
                "src/index.ts(12,5): error TS2322: Type 'string' is not assignable to type 'number'."
                    .to_string()
            } else if line == 42 {
                "  9:7  warning  'unused' is assigned a value but never used  no-unused-vars"
                    .to_string()
            } else if line == 55 {
                "  # aws_instance.web will be updated in-place".to_string()
            } else if line == 56 {
                "  ~ resource \"aws_instance\" \"web\" {".to_string()
            } else if line == 70 {
                "Plan: 0 to add, 1 to change, 0 to destroy.".to_string()
            } else {
                format!("noise line {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let reduction = reduce("test-output", &input);

    assert!(reduction.omitted);
    assert!(reduction.output.contains("error TS2322"));
    assert!(reduction.output.contains("no-unused-vars"));
    assert!(reduction.output.contains("will be updated in-place"));
    assert!(reduction.output.contains("Plan: 0 to add"));
}

#[test]
fn git_reducer_preserves_hunks_and_changed_lines() {
    let input = [
        "diff --git a/src/lib.rs b/src/lib.rs",
        "index 111..222 100644",
        "--- a/src/lib.rs",
        "+++ b/src/lib.rs",
        "@@ -1,3 +1,3 @@",
        " unchanged",
        "-old",
        "+new",
        " context",
    ]
    .join("\n");

    let reduction = reduce("git", &input);

    assert_eq!(reduction.reducer, "git");
    assert!(reduction.output.contains("diff --git"));
    assert!(reduction.output.contains("@@ -1,3 +1,3 @@"));
    assert!(reduction.output.contains("-old"));
    assert!(reduction.output.contains("+new"));
}

#[test]
fn search_reducer_groups_many_matches_by_file() {
    let input = (1..=160)
        .map(|line| format!("src/lib.rs:{line}:needle {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    let reduction = reduce("search", &input);

    assert_eq!(reduction.reducer, "search");
    assert!(reduction.omitted);
    assert!(reduction.output.contains("files matched: 1"));
    assert!(reduction.output.contains("src/lib.rs"));
    assert!(
        reduction.output.contains("omitted") || reduction.output.contains("raw match lines: 160")
    );
}

#[test]
fn log_reducer_preserves_error_context() {
    let mut lines = Vec::new();
    for line in 1..=150 {
        if line == 75 {
            lines.push("2026-01-01 ERROR database refused connection".to_string());
        } else {
            lines.push(format!("2026-01-01 INFO heartbeat {line}"));
        }
    }
    let input = lines.join("\n");

    let reduction = reduce("log", &input);

    assert_eq!(reduction.reducer, "log");
    assert!(reduction.omitted);
    assert!(
        reduction
            .output
            .contains("ERROR database refused connection")
    );
    assert!(reduction.output.contains("omitted lines"));
}

#[test]
fn json_reducer_returns_shape_not_full_payload() {
    let input = serde_json::json!({
        "items": (0..20).map(|idx| serde_json::json!({
            "id": idx,
            "body": "x".repeat(200)
        })).collect::<Vec<_>>(),
        "ok": true
    })
    .to_string();

    let reduction = reduce("json", &input);

    assert_eq!(reduction.reducer, "json");
    assert!(reduction.output.contains("$.items: array(len=20)"));
    assert!(reduction.output.contains("$.ok: bool(true)"));
    assert!(!reduction.output.contains(&"x".repeat(120)));
}

#[test]
fn outline_reducer_keeps_declarations() {
    let input = [
        "use std::path::Path;",
        "",
        "// generated filler",
        "pub struct Item;",
        "impl Item {",
        "    pub fn new() -> Self { Self }",
        "}",
    ]
    .join("\n");

    let reduction = reduce("outline", &input);

    assert_eq!(reduction.reducer, "outline");
    assert!(reduction.output.contains("use std::path::Path;"));
    assert!(reduction.output.contains("pub struct Item;"));
    assert!(reduction.output.contains("impl Item"));
}

#[test]
fn outline_reducer_keeps_lockfile_package_shape() {
    let input = [
        "# This file is automatically @generated by Cargo.",
        "",
        "[[package]]",
        "name = \"alpha\"",
        "version = \"1.0.0\"",
        "checksum = \"abcdef\"",
        "",
        "[[package]]",
        "name = \"beta\"",
        "version = \"2.0.0\"",
    ]
    .join("\n");

    let reduction = reduce("outline", &input);

    assert_eq!(reduction.reducer, "outline");
    assert!(reduction.output.contains("[[package]]"));
    assert!(reduction.output.contains("name = \"alpha\""));
    assert!(reduction.output.contains("version = \"2.0.0\""));
    assert!(!reduction.output.contains("checksum"));
}

#[test]
fn browser_snapshot_reducer_preserves_key_accessible_nodes() {
    let mut lines = vec![
        "url: https://example.test/dashboard".to_string(),
        "title: Dashboard".to_string(),
        "- banner:".to_string(),
        "  - heading \"Dashboard\" [level=1]".to_string(),
        "  - link \"Settings\"".to_string(),
        "- main:".to_string(),
        "  - textbox \"Search projects\"".to_string(),
        "  - button \"Create project\"".to_string(),
        "  - alert \"Billing failed\"".to_string(),
    ];
    for idx in 1..=160 {
        lines.push(format!("  - text \"Background table row {idx}\""));
    }
    lines.push("  - button \"Retry payment\"".to_string());
    let input = lines.join("\n");

    let reduction = reduce("browser-snapshot", &input);

    assert_eq!(reduction.reducer, "browser-snapshot");
    assert!(reduction.omitted);
    assert!(reduction.output.contains("roles:"));
    assert!(reduction.output.contains("heading=1"));
    assert!(reduction.output.contains("key accessible nodes"));
    assert!(reduction.output.contains("button \"Create project\""));
    assert!(reduction.output.contains("alert \"Billing failed\""));
    assert!(reduction.output.contains("omitted"));
}

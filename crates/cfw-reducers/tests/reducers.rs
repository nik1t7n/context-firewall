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

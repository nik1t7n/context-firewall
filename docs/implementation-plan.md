# Implementation Plan

## Phase -1: Codex Output-Replacement Spike

Before Context Firewall claims hook-native enforcement, prove that a Codex hook can replace model-visible tool output.

The canary must:

1. Run a real Codex session.
2. Execute a command that emits a large unique marker.
3. Store the raw marker in the artifact store.
4. Verify the raw marker is absent from the model-visible tool result.
5. Mark the span as `replaced_tool_result` only when replacement is proven.

If the canary fails, hook-native mode remains observer-only and wrapper mode is v1.

Current implementation status:

- `cfw canary codex-hook-replacement` creates an isolated evidence workspace.
- It runs a real `codex exec` session with a unique raw marker.
- It writes project hook config, project `hooks.json`, a temporary Codex profile config, and CLI config overrides.
- It verifies hook input, hook output, final model-visible output, and Codex JSONL events.
- Real canary evidence on `codex-cli 0.139.0` is negative: the shell command appears as `command_execution`, the raw marker reaches the final model-visible response, and the `PostToolUse` hook does not run.

This is a fail-closed gate, not a soft warning. Hook-native install must remain blocked until this canary is green on the supported Codex version.

## Phase 0: Local Execution Spine

Build the real command path first:

- `cfw run -- <command>`
- raw artifact store
- SQLite span ledger
- deterministic reducer
- retrieval handle
- `cfw show`
- `cfw spans`
- `cfw receipt`
- `cfw receipt --schema`
- `cfw purge`
- raw retrieval guard for suspected secrets

## Phase 1: Reducers

Conservative deterministic reducers shipped so far:

- test output
- git output
- search results
- logs
- JSON
- file outlines
- browser snapshots
- real-output corpus coverage for Cargo, git, grep, and jq outputs

Remaining reducer work:

- broader real-output corpus across additional ecosystems
- stricter failure-preservation invariants per ecosystem

Every reducer must preserve failure-critical evidence and include a retrieval handle whenever anything is omitted.

## Phase 2: Codex Wrapper Adapter

Make wrapper mode first-class:

- `cfw doctor codex`
- AGENTS.md snippet generator
- first-run guided command
- clear advisory-mode labeling
- dry-run install
- managed block uninstall

## Phase 3: Hook-Native Adapter

Only after Phase -1 passes:

- managed hook install
- trust/load/run/replacement verification
- uninstall rollback
- delivery evidence paths

## Phase 4: Loop Detection

Started with strict duplicate-output detection:

- same command text
- same cwd
- same exit code
- same raw output hash

Remaining hardening: repo HEAD, index hash, selected env hash, policy version, and input file hashes when known.

# Implementation Plan

## Phase -1: Codex Output-Replacement Spike

Before Context Firewall claims hook-native enforcement, verify that a Codex hook can replace model-visible tool output.

The canary must:

1. Run a real Codex session.
2. Execute a command that emits a large unique marker.
3. Store the full marker output in the artifact store.
4. Verify the compact marker is present in the model-visible tool result.
5. Mark the span as `replaced_tool_result` only when replacement is proven.

Wrapper mode is the supported Codex adapter while hook-native graduates through this canary.

Current implementation status:

- `cfw canary codex-hook-replacement` creates an isolated evidence workspace.
- It runs a real `codex exec` session with a unique raw marker.
- It writes project hook config, project `hooks.json`, and an isolated temporary `CODEX_HOME` with only real auth plus minimal canary config.
- It runs with `--dangerously-bypass-hook-trust` and `--dangerously-bypass-approvals-and-sandbox` so the probe focuses on hook delivery.
- It verifies hook input, hook output, final model-visible output, and Codex JSONL events.
- Hook-native install remains canary-gated for the supported Codex version.

This is a delivery-proof gate. Hook-native install opens when this canary is green on the supported Codex version.

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
- stricter error-preservation invariants per ecosystem

Every reducer must preserve error-critical evidence and include a retrieval handle whenever anything is omitted.

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

Current duplicate-output detection uses:

- command argv
- same cwd
- same exit code
- same raw output hash
- repo HEAD when inside a git repo
- git index tree hash when available
- selected environment allowlist hash
- policy engine version and policy config hash
- direct argv input file hashes when those files are known
- explicit `--stdin-file` content hash
- command-specific dependency fingerprints for Cargo, Node package managers, and Python/pytest-style commands

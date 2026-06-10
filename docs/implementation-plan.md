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

Remaining reducer work:

- browser snapshots
- larger real-output fixture corpus
- stricter failure-preservation invariants per ecosystem

Every reducer must preserve failure-critical evidence and include a retrieval handle whenever anything is omitted.

## Phase 2: Codex Wrapper Adapter

Make wrapper mode first-class:

- `cfw doctor codex`
- AGENTS.md snippet generator
- first-run guided command
- clear advisory-mode labeling

## Phase 3: Hook-Native Adapter

Only after Phase -1 passes:

- managed hook install
- trust/load/run/replacement verification
- uninstall rollback
- delivery evidence paths

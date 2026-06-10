# Context Firewall Global Plan

## Current State

Context Firewall is a standalone Rust workspace with:

- `cfw run -- <command>` real command execution.
- local SQLite span ledger.
- raw artifact storage.
- deterministic generic, test-output, git, search, log, JSON, and outline reducers.
- `cfw show` retrieval by line range.
- `cfw spans` ledger inspection.
- `cfw purge` artifact lifecycle cleanup.
- `cfw receipt` and `cfw receipt --json`.
- `cfw receipt --schema`.
- `cfw top`.
- `cfw first-run`.
- `cfw doctor codex`.
- policy routing for git diffs, tests, search/listing output, logs, JSON, generated reads, denied paths, and binary output.
- secret-like raw output guard on `cfw show`.
- structured `argv` in span metadata.
- proof-based duplicate output detection using command, cwd, exit code, and raw output hash, applied only when the duplicate receipt is smaller than the normal reduced output.
- explicit Codex wrapper adapter installation.
- Codex wrapper dry-run and managed block uninstall.
- real `cfw canary codex-hook-replacement` command.
- hook-native install blocked until output replacement is proven.

## North Star

Stop coding agents from wasting context on logs, diffs, repeated output, huge files, and noisy tool results, while preserving full local evidence and refusing to overclaim savings.

## Non-Negotiables

- Real execution only.
- Local-first by default.
- No hidden telemetry.
- No LLM compression in core.
- No silent fallback from hook-native to wrapper mode.
- Receipts count savings only when delivery status proves the agent saw compact output.
- Reducers preserve failure-critical evidence.

## Phase -1: Codex Output-Replacement Canary

Goal: prove or disprove hook-native enforcement.

Tasks:

- Build a minimal managed Codex hook prototype. Done.
- Emit a large unique raw marker from a real Codex shell command. Done.
- Store raw output as evidence. Done.
- Return compact replacement output through `PostToolUse` hook feedback. Implemented, but not observed in `codex exec`.
- Verify raw marker is absent from the model-visible transcript/tool result. Implemented; currently fails on `codex-cli 0.139.0`.
- Record delivery status as `replaced_tool_result` only when proven.
- Add negative canary for hook failure.

Latest real result:

- Codex version: `codex-cli 0.139.0`.
- Evidence command: `cfw canary codex-hook-replacement`.
- Result: `verified=false`.
- Observed event shape: shell execution is emitted as `command_execution`.
- Hook evidence: `hook-input.json` and `hook-output.json` are absent.
- Model-visible evidence: the final response contains the raw marker and does not contain the compact marker.

Decision:

- If canary passes, hook-native becomes v1 enforcement mode.
- If canary fails, hook-native stays observer-only and wrapper mode remains v1.

## Phase 1: Local Execution Spine

Started. Current spine includes:

- split stdout and stderr artifacts instead of combined text only.
- store command argv as structured JSON.
- add session table writes.
- add `cfw spans`.
- add purge command.
- add raw retrieval guard for suspected secrets.

Current receipt contract:

- `cfw receipt --json` emits `schema_version: cfw.receipt.v1`.
- `cfw receipt --schema` emits the JSON Schema for the receipt payload.

## Phase 2: Reducer Pack

Started. Current reducer pack:

- git diff output.
- ripgrep/grep/find/tree/search output.
- JSON shape reducer.
- logs reducer.
- file outline reducer.
- browser snapshot reducer.

Remaining:

- deeper real-output fixture corpus.

Each reducer gets:

- golden fixtures from real output.
- failure-preservation invariants.
- truncation markers.
- retrieval-handle checks.

## Phase 3: Policy Engine

Started. Current policy supports:

- `cfw policy init`.
- `cfw policy check`.
- `cfw policy explain`.
- path deny rules.
- cwd-aware canonical path checks for existing path arguments.
- symlink targets resolved before deny checks.
- case-insensitive path component matching.
- generated-file rules.
- binary-output block.

Remaining:

- noninteractive `ask` failure.

## Phase 4: Loop Detection

Started. Current repeat detection uses:

- command argv.
- cwd.
- exit code.
- raw stdout/stderr hash.
- repo HEAD when inside a git repo.
- git index tree hash when available.
- selected env allowlist hash.
- policy engine version and policy config hash.
- direct argv input file hashes when the files are known.

Remaining repeat key inputs:

- stdin content hash when commands consume stdin through Context Firewall.
- command-specific dependency fingerprints for package managers and test runners.

Never label a situation "unchanged" unless the repeat key proves it.

## Phase 5: Codex Adapter

Wrapper mode:

- improve AGENTS.md managed block UX.
- add `cfw install codex --mode wrapper --dry-run`.
- add `cfw uninstall codex` for managed block removal.

Current wrapper lifecycle:

- install snippet printing.
- managed `AGENTS.md` write.
- dry-run inspection.
- managed block removal.

Hook-native mode:

- only after Phase -1 passes.
- install managed hook config.
- separate installed/trusted/loaded/ran/replacement states.
- rollback cleanly.

## Phase 6: Open Source Launch

Ship:

- GitHub Actions CI.
- signed release binaries.
- Homebrew tap.
- README demo.
- comparison table vs RTK, Headroom, Context Mode.
- security/privacy doc.
- contributing guide.

Launch claim:

> Context Firewall stops Codex from eating logs, diffs, and repeated output. Full evidence stays local. Receipts only count verified savings.

## Phase 7: Later Adapters

After Codex:

- Claude Code.
- Gemini CLI.
- Cursor/Cline rules mode.
- OpenClaw middleware.
- MCP server.

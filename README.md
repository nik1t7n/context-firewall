# Context Firewall

Context Firewall is a local-first Rust CLI for keeping noisy command output out of coding-agent context while preserving the full evidence on disk.

The first target is Codex. The first honest adapter mode is wrapper mode: agents call `cfw run -- <command>` for commands likely to produce large output. Hook-native Codex support is experimental until a real output-replacement canary proves that Codex sees the compact tool result instead of the raw output.

## Goals

- Run real commands, never mocks.
- Store raw stdout/stderr locally.
- Return deterministic compact output to the agent.
- Provide retrieval handles for exact lines.
- Produce receipts that distinguish verified savings from observed-only waste.
- Stay local-only by default.

## First useful loop

```bash
cfw first-run
cfw run -- cargo test
cfw run -- grep -R "TODO" crates
cfw run -- cat app.log
cfw run -- cat payload.json
cfw spans
cfw receipt
cfw show <span-id> --lines 120:180
cfw purge --older-than-days 14
```

## Deterministic reducers

Context Firewall does not use an LLM to decide what to hide. It classifies the command, stores the full stdout/stderr locally, and applies a deterministic reducer:

- `test-output`: preserves failures, panics, assertions, summaries, head, and tail.
- `git`: preserves diff headers, hunk headers, changed lines, and conflict markers.
- `search`: groups grep/rg/ag/ack matches by file and caps matches per file.
- `log`: preserves log edges plus severity/error context.
- `json`: returns JSON shape, collection sizes, and small scalar samples.
- `outline`: returns headings, imports, and top-level declarations for generated/lock files.

Policy blocks obvious context waste such as dependency/build path reads and binary file output before execution.

## Local evidence lifecycle

- `cfw spans` lists recent local spans from the SQLite ledger.
- `cfw show <span-id>` retrieves raw output, with `--lines A:B` for narrow evidence.
- `cfw show <span-id> --force` is required when raw output looks credential-like.
- `cfw purge --older-than-days N` or `cfw purge --all` deletes local span rows and artifact files from the active data dir.
- span metadata stores structured `argv` alongside command text, cwd, exit code, and split stdout/stderr artifact paths.
- repeated command output is deduped only when command, cwd, exit code, and raw output hash match a previous span, and the duplicate receipt is smaller than the normal reduced output.

## Codex

Wrapper mode is available now:

```bash
cfw install codex --mode wrapper
cfw install codex --mode wrapper --write-agents --dry-run
cfw install codex --mode wrapper --write-agents
cfw uninstall codex
cfw doctor codex
```

Hook-native mode is intentionally blocked until a real output-replacement canary proves that Codex sees compact output instead of raw output.

```bash
cfw install codex --mode hook-native
# HookReplacementFailed until the canary passes.
```

## Development

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

## Status

Early implementation. The real local execution path, span ledger, policy routing, receipts, Codex wrapper install, and first reducer pack are in place. Hook-native Codex enforcement is still gated on the output-replacement canary.

See [docs/global-plan.md](docs/global-plan.md) for the build plan.

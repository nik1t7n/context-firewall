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
cfw run --stdin-file payload.json -- jq '.items | length'
cfw spans
cfw receipt
cfw receipt --schema
cfw show <span-id> --lines 120:180
cfw purge --older-than-days 14
```

## Quickstart Smoke

Run these commands inside a real repository:

```bash
cargo install --path crates/cfw-cli
cfw policy init
cfw run -- cargo test
cfw run -- git diff --stat
cfw spans
cfw receipt --json
```

When a compact result omits detail, use the printed span id to retrieve the local raw evidence:

```bash
cfw show <span-id> --lines 1:80
```

## Install

From source:

```bash
cargo install --path crates/cfw-cli
```

After the first tagged GitHub release is published:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/context-firewall/context-firewall/releases/latest/download/cfw-installer.sh | sh

brew install context-firewall/tap/cfw
```

Release artifacts are built by cargo-dist for macOS, Linux, and Windows. The release workflow publishes shell, PowerShell, and Homebrew installers, sha256 sums, source tarballs, and GitHub Artifact Attestations. Homebrew tap publishing requires the repository secret `HOMEBREW_TAP_TOKEN`.

## Deterministic reducers

Context Firewall does not use an LLM to decide what to hide. It classifies the command, stores the full stdout/stderr locally, and applies a deterministic reducer:

- `test-output`: preserves failures, panics, assertions, summaries, head, and tail.
- `git`: preserves diff headers, hunk headers, changed lines, and conflict markers.
- `search`: groups grep/rg/ag/ack matches by file and caps matches per file.
- `log`: preserves log edges plus severity/error context.
- `json`: returns JSON shape, collection sizes, and small scalar samples.
- `outline`: returns headings, imports, and top-level declarations for generated/lock files.
- `browser-snapshot`: summarizes Playwright/ARIA snapshots by roles, diagnostics, and key accessible nodes.

Policy blocks obvious context waste such as dependency/build path reads and binary file output before execution.

## Local evidence lifecycle

- `cfw spans` lists recent local spans from the SQLite ledger.
- `cfw show <span-id>` retrieves raw output, with `--lines A:B` for narrow evidence.
- `cfw show <span-id> --force` is required when raw output looks credential-like.
- `cfw receipt --schema` prints the JSON Schema for `cfw receipt --json`.
- `cfw purge --older-than-days N` or `cfw purge --all` deletes local span rows and artifact files from the active data dir.
- span metadata stores structured `argv` alongside command text, cwd, exit code, and split stdout/stderr artifact paths.
- `cfw run --stdin-file <path> -- <command>` feeds exact file bytes to command stdin and includes the stdin hash in repeat evidence.
- repeated command output is deduped only when command, cwd, exit code, stdin evidence, and raw output hash match a previous span, and the duplicate receipt is smaller than the normal reduced output.

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
cfw canary codex-hook-replacement
cfw install codex --mode hook-native
# HookReplacementFailed until the canary passes.
```

Current real canary result on `codex-cli 0.139.0`: `codex exec` runs the shell command as a `command_execution` item, but the configured `PostToolUse` hook is not invoked. Context Firewall therefore keeps hook-native install fail-closed and does not claim replacement savings for Codex hook mode yet.

## Development

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

## Status

Early implementation. The real local execution path, span ledger, policy routing, receipts, Codex wrapper install, first reducer pack, and hook-native Codex canary are in place. Hook-native Codex enforcement is still gated because the current real canary is negative on `codex-cli 0.139.0`.

See [docs/global-plan.md](docs/global-plan.md) for the build plan.
See [docs/comparison.md](docs/comparison.md) for positioning against adjacent token-optimization tools.
See [SECURITY.md](SECURITY.md) for the privacy and security model.

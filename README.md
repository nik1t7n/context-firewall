<div align="center">

# Context Firewall

**A local-first token firewall for coding agents.**

Keep logs, diffs, repeated output, generated files, and giant search results out of your agent's context while preserving the full evidence on disk.

![Rust 2024](https://img.shields.io/badge/Rust-2024-f74c00?style=for-the-badge)
![Local First](https://img.shields.io/badge/local--first-evidence-2ea043?style=for-the-badge)
![Codex Ready](https://img.shields.io/badge/Codex-wrapper--ready-111827?style=for-the-badge)
![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)

<p>
  <code>cfw run -- cargo test</code><br>
  <code>cfw run -- rg "TODO" .</code><br>
  <code>cfw show &lt;span-id&gt; --lines 120:180</code>
</p>

</div>

## Why This Exists

Coding agents are getting powerful, but they still burn context on boring output:

- 500 lines of test noise to find one assertion
- huge `rg` results where only the file grouping matters
- repeated commands that print the same thing again
- giant JSON blobs where the shape matters more than the payload
- lockfiles, generated files, build folders, and logs

Context Firewall sits between the agent and the terminal. It runs the real command, stores the full output locally, and returns a compact version with a span handle for exact retrieval.

## Measured Impact

Measured locally on this repository with `cfw 0.1.0` in wrapper mode.

| Real command | Raw estimated tokens | Returned estimated tokens | Reduction |
| --- | ---: | ---: | ---: |
| `rg -n 'Context Firewall\|Codex\|receipt\|span\|policy\|canary' docs crates` | 9,962 | 1,624 | 83.69% |
| `sed -n '1,220p' docs/global-plan.md` | 1,610 | 1,124 | 30.18% |
| `cargo test` | 1,298 | 1,001 | 22.88% |
| Three-command local measurement | 12,870 | 3,749 | 70.87% |

Receipt from the same run:

```json
{
  "spans": 3,
  "raw_estimated_tokens": 12870,
  "returned_estimated_tokens": 3749,
  "net_estimated_saved": 9121
}
```

Every number above was produced by real CLI runs on this repo.

## The Loop

```bash
# install from source
cargo install --path crates/cfw-cli

# run noisy commands through the firewall
cfw run -- cargo test
cfw run -- rg -n "TODO|FIXME" crates docs
cfw run -- git diff
cfw run -- cat app.log

# inspect what happened
cfw spans
cfw receipt --json

# pull exact raw evidence when you need it
cfw show <span-id> --lines 120:180
```

The agent sees compact output. You keep the raw artifact.

## What the Agent Gets

Instead of dumping a full noisy result into context, Context Firewall returns the useful part plus a handle:

```text
[context-firewall: search summary]
files matched: 18
raw match lines: 457

crates/cfw-cli/src/main.rs
  9:use cfw_core::receipt::{RECEIPT_SCHEMA_JSON, RECEIPT_SCHEMA_VERSION};
  10:use cfw_core::span::{DeliveryStatus, SpanRecord};

[context-firewall]
span: cfw://span/019eb4cb697a76208ba7a71ff72b51d7
raw: 39,848 bytes, estimated 9,962 tokens
returned: 6,496 bytes, estimated 1,624 tokens
full output stored locally
[/context-firewall]
```

Need more? Ask for the exact lines:

```bash
cfw show 019eb4cb697a76208ba7a71ff72b51d7 --lines 120:180
```

## Codex Setup

Context Firewall is Codex-first.

```bash
cfw install codex --mode wrapper --write-agents --dry-run
cfw install codex --mode wrapper --write-agents
cfw doctor codex
```

That adds a managed `AGENTS.md` block telling Codex when to use `cfw run -- ...`.

Hook-native mode is prepared behind a real output-replacement canary:

```bash
cfw canary codex-hook-replacement
cfw install codex --mode hook-native
```

Wrapper mode is available today. Hook-native mode graduates when the canary verifies compact model-visible delivery on a supported Codex version.

## Built For Trust

Context Firewall keeps the core path solid:

- real commands in, compact output out
- raw stdout and stderr stored locally
- deterministic reducers
- span handles for exact evidence
- SQLite ledger for receipts
- policy gates for obvious context waste
- repeat fingerprints for safe duplicate suppression
- local lifecycle controls with `cfw purge`

It is a local evidence layer with a compact delivery path.

## Reducers

| Reducer | What it keeps |
| --- | --- |
| `test-output` | test errors, panics, assertions, summaries, head, tail |
| `git` | file headers, hunks, changed lines, conflict markers |
| `search` | files matched, match counts, capped examples per file |
| `log` | severity lines, error context, head, tail |
| `json` | object shape, collection sizes, scalar samples |
| `outline` | headings, imports, declarations, package names |
| `browser-snapshot` | roles, diagnostics, key accessible nodes |

## Receipts

Use receipts to see what Context Firewall did:

```bash
cfw receipt
cfw receipt --json
cfw receipt --schema
```

Receipts separate observed token waste from delivery-backed savings. Wrapper mode reports what it returned through `cfw run`; hook-native mode will report replacement-backed savings after the canary verifies that path.

## Release Readiness

The project already has:

- GitHub Actions CI for `fmt`, tests, and clippy
- cargo-dist release packaging
- macOS Apple Silicon, macOS Intel, Linux x64, Linux ARM64, and Windows x64 artifacts
- shell and PowerShell installers
- Homebrew tap generation
- GitHub Artifact Attestations
- release smoke workflow that downloads the published artifact and runs the real binary

Local release smoke:

```bash
cargo build -p cfw
scripts/release-smoke.sh target/debug/cfw
```

## Development

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
scripts/release-smoke.sh target/debug/cfw
```

Useful targeted tests:

```bash
cargo test -p cfw --test cli repeated_identical_command_returns_duplicate_handle
cargo test -p cfw --test cli changed_stdin_file_prevents_duplicate_handle_even_with_same_output
cargo test -p cfw --test cli changed_cargo_lock_prevents_duplicate_handle_even_with_same_output
cargo test -p cfw-reducers --test real_corpus
```

## Roadmap

- Codex wrapper mode: available
- Codex hook-native mode: canary-gated
- Claude Code adapter: next
- Gemini CLI, Cursor, OpenClaw, MCP: later

## More

- [Codex hook-native roadmap](docs/codex-hook-native-roadmap.md)
- [Global build plan](docs/global-plan.md)
- [Comparison with adjacent tools](docs/comparison.md)
- [Security model](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

<div align="center">

# Context Firewall

### Stop wasting agent context on terminal noise.

Coding agents waste scarce context on terminal noise: logs, diffs, test output,
and search results crowd out the code they need to understand.

Context Firewall runs real commands, gives agents the compact signal, and keeps
full raw evidence on disk for exact retrieval.

![Rust 2024](https://img.shields.io/badge/Rust-2024-f74c00?style=for-the-badge)
![Local First](https://img.shields.io/badge/local--first-2ea043?style=for-the-badge)
![Agent Ready](https://img.shields.io/badge/agent-ready-111827?style=for-the-badge)
![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)

<br />

<strong>Less noise in context. More room for the work.</strong>

</div>

---

## The Problem

Coding agents are powerful until your context gets filled with junk:

- 500 lines of test output to find one failed assertion
- giant `rg` results where only the file map matters
- huge diffs pasted into the model again and again
- logs, generated files, lockfiles, snapshots, and JSON blobs
- repeated commands that say the same thing twice

Every wasted token competes with the code, plan, bug, or decision you actually
needed the agent to understand.

Context Firewall puts a clean boundary between command output and agent context.

## What It Does

```text
Your command
    |
    v
cfw run -- cargo test
    |
    +-- stores full stdout/stderr locally
    |
    +-- returns the useful summary to the agent
    |
    +-- gives you a span handle for exact retrieval
```

The agent sees the signal. You keep the evidence.

## Quick Start

```bash
# macOS / Linux, after the first tagged release
brew install nik1t7n/tap/cfw

# npm / npx, after the first tagged release
npm install -g @nik1t7n/context-firewall
npx @nik1t7n/context-firewall --help

# from this checkout today
cargo install --path crates/cfw-cli

cfw install agent
```

Then run noisy commands through the firewall:

```bash
cfw run -- cargo test
cfw run -- rg -n "TODO|FIXME" crates docs
cfw run -- git diff
cfw run -- cat app.log
```

Need the original output?

```bash
cfw spans
cfw show <span-id> --lines 120:180
cfw receipt --json
```

## Agent Integrations

Context Firewall speaks MCP and ships installers for the agent surfaces people
actually use.

```bash
# Generic AGENTS.md instructions
cfw install agent

# Gemini CLI: .gemini/settings.json + GEMINI.md
cfw install gemini

# Google Antigravity: project config + local Antigravity MCP configs
cfw install antigravity

# Claude Code: .mcp.json + AGENTS.md + CLAUDE.md import
cfw install claude

# Cursor: .cursor/mcp.json + .cursor/rules/context-firewall.mdc + AGENTS.md
cfw install cursor
```

All MCP clients connect to the same local server:

```bash
cfw mcp
```

Tools exposed over MCP:

| Tool | Purpose |
| --- | --- |
| `cfw_run` | run a real command through Context Firewall |
| `cfw_show` | retrieve exact stored span output |
| `cfw_spans` | list recent spans |
| `cfw_receipt` | inspect recent token accounting |

## The Aha Moment

Instead of feeding the agent a wall of output:

```text
docs/global-plan.md:...
docs/comparison.md:...
crates/cfw-cli/src/main.rs:...
crates/cfw-cli/tests/cli.rs:...
... hundreds of lines ...
```

Context Firewall returns the shape of the result:

```text
[context-firewall: search summary]
files matched: 19
raw match lines: 499

README.md
  3:# Context Firewall
  32:Context Firewall puts a clean boundary...

crates/cfw-cli/src/main.rs
  10:use cfw_core::receipt::{...}
  54:Run a guided local command...

[context-firewall]
span: cfw://span/019ecaf492c07370a55c6943fc98021b
raw: 43,512 bytes, estimated 10,878 tokens
returned: 6,870 bytes, estimated 1,718 tokens
full output stored locally
[/context-firewall]
```

If the agent needs more, it asks for the exact span lines instead of rerunning
the command or flooding the conversation.

## Real Local Impact

Measured on this repository with `cfw 0.1.0`.

| Command | Raw estimated tokens | Returned estimated tokens | Reduction |
| --- | ---: | ---: | ---: |
| Repository search across docs, crates, and README | 10,878 | 1,718 | 84.21% |
| `cargo test` | 1,319 | 999 | 24.26% |
| Local two-command session | 12,197 | 2,717 | 77.72% |

Receipt:

```json
{
  "spans": 2,
  "raw_estimated_tokens": 12197,
  "returned_estimated_tokens": 2717,
  "net_estimated_saved": 9480
}
```

## Why Developers Use It

| Without Context Firewall | With Context Firewall |
| --- | --- |
| Agent context fills with command noise | Agent sees compact, task-relevant output |
| Raw evidence disappears into scrollback | Raw evidence is stored locally |
| Repeated commands waste more tokens | Duplicate output can collapse to a handle |
| Big diffs and logs derail the turn | Summaries stay readable |
| Debugging requires reruns | Exact lines are retrievable by span |

## Built For Coding Agents

Context Firewall is agent-facing infrastructure, not a log viewer.

- MCP server for agent tools
- Installers for Gemini CLI, Antigravity, Claude Code, Cursor, and generic agents
- Real command execution
- Local raw artifacts
- Deterministic reducers
- Span handles for exact evidence
- Receipts for token accounting
- Policy gates for obvious context waste
- No cloud account
- No hosted proxy
- No raw output loss

## Reducers

Context Firewall understands the noisy shapes agents hit every day:

| Reducer | Keeps |
| --- | --- |
| `test-output` | failures, panics, assertions, summaries, head, tail |
| `git` | file headers, hunks, changed lines, conflict markers |
| `search` | matched files, counts, capped examples per file |
| `log` | severity lines, error context, head, tail |
| `json` | object shape, collection sizes, scalar samples |
| `outline` | headings, imports, declarations, package names |
| `browser-snapshot` | roles, diagnostics, key accessible nodes |

## Common Workflow

```bash
# run the real command
cfw run -- cargo test

# inspect recent spans
cfw spans

# retrieve exact evidence
cfw show 019ecaf49aab746395d2e02d31fa5d76 --lines 40:90

# see context savings
cfw receipt
```

## Install For Development

```bash
cargo build -p cfw
cargo test
cargo clippy -- -D warnings
```

Release smoke:

```bash
scripts/release-smoke.sh target/debug/cfw
```

See [INSTALL.md](INSTALL.md) for Homebrew, npm/npx, shell installer,
PowerShell, Rust, and release-owner setup.

## Project Status

Context Firewall is early, useful, and intentionally small. It is built for the
loop every coding agent lives in: read, search, test, diff, fix, repeat.

The broader goal is simple:

> Give every coding agent a clean context boundary for terminal output.

## Links

- [Comparison](docs/comparison.md)
- [Security model](SECURITY.md)
- [Contributing](CONTRIBUTING.md)
- [Implementation notes](docs/implementation-plan.md)

## License

Apache-2.0

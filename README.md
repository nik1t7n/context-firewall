<div align="center">

# Context Firewall

### Stop wasting agent context on terminal noise.

Coding agents waste scarce context on terminal noise: logs, diffs, test output,
and search results crowd out the code they need to understand.

Context Firewall solves this by giving agents the compact signal and keeping
raw evidence available for precise retrieval, saving up to 94.6% of noisy
command context for useful work.

![Rust 2024](https://img.shields.io/badge/Rust-2024-f74c00?style=for-the-badge)
![Local First](https://img.shields.io/badge/local--first-2ea043?style=for-the-badge)
![Agent Ready](https://img.shields.io/badge/agent-ready-111827?style=for-the-badge)
![License](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)

<br />

<strong>Less noise in context. More room for the work.</strong>

Real release audit: <strong>300k raw tokens became 16k agent-visible tokens</strong>
while every full command output stayed retrievable on disk.

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

Need the full command reference? See the [usage guide](docs/usage.md).

## Features

- **Less context waste**: agents see the useful part of tests, diffs, searches,
  logs, and big files instead of the whole wall of output.
- **Full evidence stays safe**: raw stdout/stderr is kept locally, so the agent
  can pull the exact proof later.
- **Automatic routing**: supported agents can route noisy commands through
  Context Firewall without relying on memory or discipline.
- **Searchable history**: find the error, line, JSON value, or old command output
  without rerunning the command.
- **Clear value numbers**: see how many tokens were saved, which commands burn
  context, and whether agents are actually using the tool.
- **Self-improving workflow**: Context Firewall can point out repeated misses and
  suggest better project rules.
- **Works with real coding noise**: tests, diffs, search results, logs, JSON,
  Docker, Kubernetes, Terraform, TypeScript, ESLint, and GitHub CLI output.
- **Local by default**: no cloud account, no hosted proxy, no raw output loss.

## Quick Start For Agents

If you are a human, paste this prompt into the coding agent you use for your
repo. If you are the agent, read it and execute it.

```text
Set up Context Firewall for this repository.

Goal: from now on, route noisy terminal output through Context Firewall so I
keep compact signal in context and raw stdout/stderr on disk for exact
retrieval.

1. Check whether `cfw` is installed:
   - Run `cfw --version`.
   - If it works, continue.
   - If it is missing, install it through the first real path available here:
     - In the Context Firewall source checkout: `cargo install --path crates/cfw-cli`
     - macOS/Linux after release: `brew install nik1t7n/tap/cfw`
     - npm/npx after release: `npm install -g @nik1t7n/context-firewall`
     - GitHub release installer:
       `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/nik1t7n/context-firewall/releases/latest/download/cfw-installer.sh | sh`
     - Windows PowerShell after release:
       `irm https://github.com/nik1t7n/context-firewall/releases/latest/download/cfw-installer.ps1 | iex`
   - Do not pretend it worked. If every real install path fails, stop and
     report the exact blocker.

2. Install the adapter for the agent host you are running in:
   - Codex wrapper guidance: `cfw install codex --write-agents`
   - Codex hook-native rewrite where supported:
     `cfw install codex --mode hook-native --write-agents`
   - Gemini CLI: `cfw install gemini`
   - Antigravity: `cfw install antigravity`
   - Claude Code: `cfw install claude`
   - Cursor: `cfw install cursor`
   - Unknown or generic agent: `cfw install agent`

3. Verify the setup:
   - Run `cfw receipt --json`.
   - If MCP tools are available, call `cfw_receipt`.
   - Confirm that `cfw_run`, `cfw_show`, `cfw_search`, `cfw_gain`,
     `cfw_discover`, `cfw_session`, `cfw_spans`, and `cfw_receipt` are
     available, or that `cfw run -- <command>` works from the shell.

4. Use Context Firewall from now on:
   - Tests: `cfw_run` or `cfw run -- cargo test`
   - Search: `cfw_run` or `cfw run -- rg -n "TODO|FIXME" .`
   - Diffs: `cfw_run` or `cfw run -- git diff`
   - Logs and large files: `cfw_run` or `cfw run -- cat app.log`

5. When exact output is needed:
   - List spans: `cfw_spans` or `cfw spans`
   - Retrieve lines: `cfw_show` or `cfw show <span-id> --lines 120:180`
   - Search raw evidence: `cfw show <span-id> --grep "panic" --around 3`
   - Extract JSON: `cfw show <span-id> --json-path '$.errors[0]'`
   - Check savings: `cfw_gain`, `cfw_session`, or `cfw receipt --json`
```

Context Firewall uses MCP over stdio, so compatible agent clients can run
`cfw mcp` locally and call the tools directly.

## Agent Integrations

Context Firewall speaks MCP and ships installers that agents can run inside the
project they are working on.

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
| `cfw_run` | run commands without flooding context |
| `cfw_show` | pull exact proof from stored output |
| `cfw_search` | search previous command output |
| `cfw_gain` | show saved context |
| `cfw_discover` | find commands that waste context |
| `cfw_session` | show adoption and quality |
| `cfw_spans` | list recent command outputs |
| `cfw_receipt` | show token accounting |

## The Aha Moment

Instead of feeding the agent a wall of output:

```text
README.md:...
INSTALL.md:...
crates/cfw-cli/src/main.rs:...
crates/cfw-cli/tests/cli.rs:...
... hundreds of lines ...
```

Context Firewall returns the shape of the result:

```text
[context-firewall: search summary]
files matched: 18
raw match lines: 318

README.md
  3:# Context Firewall
  40:Context Firewall puts a clean boundary...

crates/cfw-cli/src/main.rs
  10:use cfw_core::receipt::{...}
  54:Run a guided local command...

[context-firewall]
span: cfw://span/019ecaf492c07370a55c6943fc98021b
raw: 26,989 bytes, estimated 6,748 tokens
returned: 4,353 bytes, estimated 1,089 tokens
full output stored locally
[/context-firewall]
```

If the agent needs more, it asks for the exact span lines instead of rerunning
the command or flooding the conversation.

## Outstanding In A Real Agent Run

Measured on this repository with the public `cfw 0.1.0` release.

The benchmark used the kind of noisy commands agents actually run during a
release audit: repo-wide search, a full release patch, `cargo metadata`, source
file dumps, workspace tests, clippy, and GitHub Actions job JSON.

| Run | Raw estimated tokens | Agent-visible tokens | Saved | Reduction |
| --- | ---: | ---: | ---: | ---: |
| Direct CFW benchmark | 300,794 | 16,448 | 284,346 | 94.53% |
| Codex CLI agent | 300,156 | 16,073 | 284,083 | 94.65% |
| Gemini CLI agent | 300,191 | 16,097 | 284,094 | 94.64% |

That is the point: the agent keeps the compact signal in context, and the full
raw stdout/stderr stays on disk for exact `cfw show <span-id>` retrieval.

One standout span:

```text
cargo metadata --format-version 1 --all-features
raw:      160,887 estimated tokens
returned:   5,617 estimated tokens
saved:    155,270 estimated tokens
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

- Agent tools and installers for common coding agents
- Automatic command routing where supported
- Real command execution, never pretend output
- Local raw evidence
- Compact summaries for noisy commands
- Exact retrieval when the agent needs proof
- Search across previous command output
- Analytics for savings, adoption, and weak spots
- Suggestions from repeated misses
- Guardrails for obvious context waste
- No cloud account
- No hosted proxy
- No raw output loss

## How It Handles Noise

Context Firewall does not try to perfectly understand every possible terminal
format. That would be brittle.

Instead, it keeps the important signals, stores the full evidence, and measures
when the summary was not good enough. If agents keep asking for the raw output
or rerunning the same command, that is a sign the project rules should improve.

## Common Workflow

```bash
# run the real command
cfw run -- cargo test

# inspect recent spans
cfw spans

# retrieve exact evidence
cfw show 019ecaf49aab746395d2e02d31fa5d76 --lines 40:90

# search raw evidence without knowing line numbers
cfw show 019ecaf49aab746395d2e02d31fa5d76 --grep "panic" --around 3

# extract JSON from stored raw output
cfw show 019ecaf49aab746395d2e02d31fa5d76 --json-path '$.errors[0]'

# see context savings and adoption
cfw gain
cfw session --reducers

# discover weak spots and suggested rules
cfw discover
cfw learn
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
PowerShell, Rust, and maintainer release setup.

## Project Status

Context Firewall is early, useful, and intentionally small. It is built for the
loop every coding agent lives in: read, search, test, diff, fix, repeat.

The broader goal is simple:

> Give every coding agent a clean context boundary for terminal output.

## Links

- [Comparison](docs/comparison.md)
- [Usage guide](docs/usage.md)
- [Security model](SECURITY.md)
- [Contributing](CONTRIBUTING.md)
- [Install guide](INSTALL.md)

## License

Apache-2.0

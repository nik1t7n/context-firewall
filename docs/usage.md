# Context Firewall Usage Guide

This is the practical reference for using Context Firewall after it is
installed.

## Mental Model

Context Firewall sits between a coding agent and noisy terminal output.

When you run:

```bash
cfw run -- cargo test
```

Context Firewall:

1. runs the real command;
2. stores full stdout/stderr locally;
3. returns a compact summary to the agent;
4. prints a span id that can retrieve the raw output later.

The short version: keep context clean, keep evidence available.

## Install Agent Integration

Generic instructions:

```bash
cfw install agent
```

Codex wrapper instructions:

```bash
cfw install codex --write-agents
```

Codex auto-routing where supported:

```bash
cfw install codex --mode hook-native --write-agents
```

Other agent hosts:

```bash
cfw install gemini
cfw install antigravity
cfw install claude
cfw install cursor
```

Preview changes without writing:

```bash
cfw install codex --mode hook-native --dry-run
```

Remove managed Codex integration files/blocks:

```bash
cfw uninstall codex
```

Check local health:

```bash
cfw doctor
cfw doctor codex
```

## Run Commands

Default usage:

```bash
cfw run -- cargo test
cfw run -- git diff
cfw run -- rg -n "TODO|FIXME" .
cfw run -- docker logs api
```

Force a reducer kind when you know the output shape:

```bash
cfw run --kind test-output -- cargo test
cfw run --kind git -- git diff
cfw run --kind search -- rg -n "auth error" .
cfw run --kind large_log -- kubectl logs deploy/api
```

Pass exact stdin from a file:

```bash
cfw run --stdin-file input.txt -- some-command
```

## Retrieve Raw Evidence

List recent spans:

```bash
cfw spans
cfw spans --json
```

Show full raw output:

```bash
cfw show <span-id>
```

Show exact lines:

```bash
cfw show <span-id> --lines 40:90
```

Search inside one span:

```bash
cfw show <span-id> --grep "panic" --around 3
```

Extract from JSON output:

```bash
cfw show <span-id> --json-path '$.errors[0]'
```

Search recent raw outputs:

```bash
cfw search-spans "connection refused"
cfw search-spans "panic" --limit 200
```

If output looks secret-like, `show` and `search-spans` stop by default. Use
`--force` only when you intentionally want to print that raw output.

## Measure Value

Token accounting:

```bash
cfw receipt
cfw receipt --json
```

Savings:

```bash
cfw gain
cfw gain --limit 500
```

Find weak spots:

```bash
cfw discover
```

Session adoption and reducer mix:

```bash
cfw session
cfw session --reducers
```

`cfw session --reducers` shows whether summaries are actually useful. High raw
fetch or rerun rates usually mean the project needs better rules for that kind
of output.

## Learn From Misses

```bash
cfw learn
```

`cfw learn` reads the local ledger and prints suggestions. It does not edit
files. It looks for repeated failures, repeated raw lookups, low-savings
summaries, and repeated large commands.

## Project-Specific Output Rules

If a project has a noisy command that Context Firewall does not summarize well,
add a project rule in `.cfw/reducers.toml`.

```toml
[[reducers]]
name = "terraform-plan"
match_command = "^terraform plan"
strip_lines_matching = ["^Refreshing state", "^Reading\\.\\.\\."]
keep_lines_matching = ["Error:", "Plan:", "^  #"]
max_lines = 80
tail_lines = 20
on_empty = "terraform plan: no relevant output"
```

Supported fields:

- `name`
- `match_command`
- `strip_lines_matching`
- `keep_lines_matching`
- `max_lines`
- `tail_lines`
- `on_empty`

Project rules apply to default `cfw run` output. Explicit `--kind` uses the
built-in reducer you requested.

## Built-In Output Handling

Context Firewall has built-in handling for common coding-agent noise:

- tests and build output;
- git diffs;
- search results;
- logs;
- JSON;
- source outlines and generated files;
- browser snapshots;
- Docker and Kubernetes logs;
- Terraform plans;
- TypeScript and ESLint output;
- GitHub CLI PR output.

The design is conservative: keep likely signal, store full raw evidence, and
let the agent retrieve exact proof later.

## MCP Tools

Run the local MCP server:

```bash
cfw mcp
```

Available tools:

| Tool | Use |
| --- | --- |
| `cfw_run` | Run a real command through Context Firewall |
| `cfw_show` | Retrieve full output, lines, grep matches, or JSON paths |
| `cfw_search` | Search recent raw outputs |
| `cfw_gain` | Show savings |
| `cfw_discover` | Find low-signal commands |
| `cfw_session` | Show adoption and quality stats |
| `cfw_spans` | List recent spans |
| `cfw_receipt` | Show token accounting |

## Policy

Create the default policy file:

```bash
cfw policy init
```

Validate it:

```bash
cfw policy check
```

Explain how a command would be classified:

```bash
cfw policy explain -- cargo test
```

Policy is for obvious context-waste guardrails. It does not replace running the
real command.

## Compact Stdin

You can compact existing text through a reducer:

```bash
cat output.txt | cfw compact --kind test-output
```

This does not create a span. Use `cfw run` when you want stored evidence and
retrieval.

## Canaries

Run real integration checks:

```bash
cfw canary codex
```

Optional Codex binary/model:

```bash
cfw canary codex --codex-bin codex --model <model>
```

## Updates

Check whether a newer release is available:

```bash
cfw update-check --force
```

In an interactive terminal, Context Firewall also checks at most once a day and
prints a short upgrade note when a newer release exists. Set
`CFW_NO_UPDATE_CHECK=1` to disable the reminder.

## Common Recipes

Run tests without flooding context:

```bash
cfw run -- cargo test
```

Debug a failed test:

```bash
cfw show <span-id> --grep "FAILED" --around 5
```

Review a big diff:

```bash
cfw run -- git diff
cfw show <span-id> --grep "fn " --around 2
```

Inspect recent value:

```bash
cfw gain
cfw session --reducers
```

Find rules to improve:

```bash
cfw discover
cfw learn
```

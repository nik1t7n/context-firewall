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
cfw run -- cargo test
cfw receipt
cfw show <span-id> --lines 120:180
```

## Status

Early implementation. The project intentionally starts with the real local execution path before any hosted service, cloud telemetry, or LLM-based compression.


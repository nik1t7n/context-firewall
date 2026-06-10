# Contributing

Context Firewall is strict about trust because it handles raw command output that can contain private code and secrets.

## Development Loop

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

## Rules

- Use real command outputs for fixtures.
- Label fixtures as fixtures.
- Do not add LLM-based reducers to core.
- Do not add telemetry without an explicit design review.
- Do not count observed-only spans as saved tokens.
- Preserve full raw artifacts unless a feature explicitly purges them.


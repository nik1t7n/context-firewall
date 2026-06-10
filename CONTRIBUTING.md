# Contributing

Context Firewall is strict about trust because it handles raw command output that can contain private code and secrets.

## Principles

- Build the real path.
- Do not add demos, mocks, fallbacks, or synthetic stand-ins unless a test explicitly labels them as fixtures.
- Keep raw evidence retrievable unless the user explicitly purges it.
- Do not claim savings that are not proven by delivery evidence.
- Prefer small deterministic reducers over opaque compression.
- Fail closed when an adapter cannot prove interception or replacement.

## Development Loop

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

## Release Checks

Before a release PR or tag:

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
dist plan --allow-dirty --output-format=json
```

For a local macOS smoke build:

```bash
dist build --allow-dirty --target aarch64-apple-darwin --artifacts=local --output-format=json
```

Use the host target that matches the release machine when testing on Intel macOS or Linux.

## Fixtures

- Use real command outputs for fixtures.
- Label fixtures as fixtures.
- Keep fixtures minimal and scrubbed.
- Do not invent command output to make a reducer look better.

## Reducers

- Preserve failure-critical evidence.
- Include omission markers when output is truncated.
- Keep retrieval handles visible when raw output is stored elsewhere.
- Add real-output corpus coverage for new ecosystems.

## Adapters

- Codex hook-native mode must stay blocked until `cfw canary codex-hook-replacement` proves output replacement.
- Adapter install paths must distinguish advisory, observed, and replaced delivery states.
- Do not silently downgrade from hook-native to wrapper mode.

## Security

- Do not add LLM-based reducers to core.
- Do not add telemetry without an explicit design review.
- Do not count observed-only spans as saved tokens.
- Preserve full raw artifacts unless a feature explicitly purges them.
- Treat raw command output as potentially secret.

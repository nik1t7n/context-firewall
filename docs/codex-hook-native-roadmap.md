# Codex Hook-Native Roadmap

Context Firewall's current Codex adapter is wrapper mode: agents route noisy commands through `cfw run -- <command>`, get compact output back, and keep full evidence locally.

Hook-native mode is the next Codex integration track. It is protected by a real canary so the project only advertises replacement-backed savings when Codex confirms the compact hook feedback is the model-visible tool result.

## Public Contract

OpenAI's Codex Hooks documentation describes `PreToolUse` and `PostToolUse` hook events, `Bash` matchers, command handlers, hook trust, and the `--dangerously-bypass-hook-trust` automation flag.

Source: https://developers.openai.com/codex/hooks

## Implementation Signals

The installed Codex binary and current OpenAI Codex source include the relevant hook runtime pieces:

- `core/src/hook_runtime.rs`
- `hooks/src/events/pre_tool_use.rs`
- `hooks/src/events/post_tool_use.rs`
- `hooks/src/engine/dispatcher.rs`
- `hooks/src/engine/command_runner.rs`

The intended integration path is clear:

1. Tool handlers expose hook payloads.
2. `core/src/tools/registry.rs` runs `PreToolUse` before tool execution.
3. Successful tool output flows through `PostToolUse`.
4. `PostToolUse` feedback can become the model-visible tool response.

That is the right long-term shape for Context Firewall: store full output locally, return compact feedback through the hook result, and mark receipts as replacement-backed only when the canary verifies that delivery path.

## Canary Gate

Command:

```bash
cfw canary codex-hook-replacement
```

The canary uses:

- isolated temporary `CODEX_HOME`
- copied real Codex auth
- `[features].hooks = true`
- trusted project config
- `--enable hooks`
- `--dangerously-bypass-hook-trust`
- `--dangerously-bypass-approvals-and-sandbox`
- a unique raw marker and compact marker
- Codex JSONL event capture

The gate checks four things:

- Codex ran the hook.
- The hook saw the raw tool output.
- The hook emitted compact feedback.
- The final model-visible result contains the compact marker.

When all four are true on a supported Codex version, hook-native can become an installable adapter.

## Product Path

Supported now:

```bash
cfw run -- <command>
```

Prepared next:

```bash
cfw canary codex-hook-replacement
cfw install codex --mode hook-native
```

The adapter boundary stays simple: wrapper mode is production-ready today; hook-native mode graduates when the real canary verifies model-visible replacement.

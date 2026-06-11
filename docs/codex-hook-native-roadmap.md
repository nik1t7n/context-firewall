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

The intended integration path is still the right target:

1. Tool handlers expose hook payloads.
2. `core/src/tools/registry.rs` runs `PreToolUse` before tool execution.
3. Successful tool output flows through `PostToolUse`.
4. Context Firewall stores full output locally and returns compact model-visible output.

Context Firewall should only mark receipts as replacement-backed when a real Codex turn proves that compact output, not raw output, reached the model.

## Current Evidence

Date: June 11, 2026

Environment:

- Codex CLI: `codex-cli 0.139.0`
- Platform: macOS arm64
- Official docs checked: https://developers.openai.com/codex/hooks
- Real probes used isolated temporary `CODEX_HOME` directories, copied real Codex auth, real model turns, unique raw markers, and unique compact markers.

Observed behavior:

| Surface | Probe | Result |
| --- | --- | --- |
| `codex exec` | `hooks.json`, `PreToolUse`, `PostToolUse`, `--enable hooks`, `--dangerously-bypass-hook-trust` | Hook command was not invoked; raw shell output reached the model. |
| `codex exec` | Same probe with `--disable unified_exec` | Hook command was not invoked; raw shell output reached the model. |
| `codex exec` | Inline TOML hooks instead of `hooks.json` | Hook command was not invoked; raw shell output reached the model. |
| `codex exec` | `features.codex_hooks = true` alias plus `features.hooks = true` | Hook command was not invoked; raw shell output reached the model. |
| app-server / Desktop-style catalog | JSON-RPC `hooks/list` | Hook was discovered and listed as enabled but untrusted, which confirms config discovery works. |
| app-server / Desktop-style turn | Real `thread/start` and `turn/start` with shell command | Hook command was not invoked; raw shell output reached the model. |
| app-server / Desktop-style turn | Same probe with `--disable unified_exec` | Hook command was not invoked; raw shell output reached the model. |

Representative final model-visible output from the app-server turn:

```text
RESULT=RAW_APP_NO_UNIFIED_SHOULD_NOT_REACH
```

Representative command event:

```json
{
  "type": "commandExecution",
  "command": "/bin/zsh -lc 'cat raw-marker.txt'",
  "aggregatedOutput": "RAW_APP_NO_UNIFIED_SHOULD_NOT_REACH\n"
}
```

No `PreToolUse-*.json` or `PostToolUse-*.json` hook input files were produced in those real turn probes.

Conclusion: Codex currently discovers hook configuration through the app-server catalog, but the tested shell execution paths do not invoke lifecycle hooks for model-generated shell commands. Hook-native Context Firewall should stay canary-gated until a future Codex build changes that behavior.

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

The gate checks five things:

- Codex ran the hook.
- The hook saw the raw tool output.
- The hook emitted compact feedback.
- The final model-visible result contains the compact marker.
- The final model-visible result does not contain the raw marker.

When all five are true on a supported Codex version, hook-native can become an installable adapter.

## Future Probe Plan

Re-run hook-native evaluation when Codex ships hook changes or the public docs mention shell lifecycle hook execution fixes.

The next useful probes are:

- `cfw canary codex-hook-replacement` against the new Codex version.
- A `PreToolUse` rewrite probe that changes `cat raw-marker.txt` into `cfw run -- cat raw-marker.txt`.
- A `PostToolUse` compact-output probe that proves the model sees only the compact marker.
- An app-server / Desktop-style JSON-RPC turn probe after confirming `hooks/list` still discovers the hook.
- A plugin-bundled hook probe only after the direct user-config hook probe succeeds.

Do not enable `cfw install codex --mode hook-native` until the real canary verifies compact model-visible delivery.

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

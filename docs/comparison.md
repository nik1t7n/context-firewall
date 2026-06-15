# Comparison

Context Firewall is intentionally narrower than broad context-management
products. It focuses on evidence-preserving command-output compaction for
coding agents.

## Positioning

| Project | Public positioning | Main surface | Context Firewall difference |
| --- | --- | --- | --- |
| Terminal-output compressors | Reduce large command output before it reaches an agent. | CLI wrappers, shell integrations, or agent-specific adapters. | Context Firewall stores the full raw evidence locally and returns span handles for exact retrieval. |
| Broad context managers | Compress files, chats, retrieval chunks, memory, and tool output. | Platform services, proxies, SDKs, and hosted systems. | Context Firewall stays local-first and only handles the command-output path agents already use. |
| Agent-specific rules | Tell one agent to summarize or avoid noisy commands. | Prompt rules, IDE settings, or project memory files. | Context Firewall gives multiple agents the same MCP tools, local ledger, reducers, and receipt format. |

## Design Tradeoffs

Context Firewall optimizes for trust over maximum headline savings:

- Raw evidence stays queryable with span handles.
- Reducers are deterministic and easy to audit.
- Policy can block waste before command execution.
- Duplicate suppression requires a repeat fingerprint, not a vague "same task" guess.
- Savings claims are tied to stored receipts.

This means Context Firewall may report lower savings than tools that compress
more aggressively or route more surfaces. That is acceptable. The core promise
is that the compact output remains backed by local raw evidence.

## Current Limits

- Context Firewall focuses on command output, not conversation history,
  retrieval chunks, or cross-agent memory.
- Agents still need to route noisy commands through `cfw_run` or
  `cfw run -- <command>`.
- The highest savings come from noisy searches, diffs, logs, repeated output,
  and long test runs.

# Comparison

Context Firewall is intentionally narrower than broad context-management products. It focuses on evidence-preserving command-output compaction for coding agents, starting with Codex wrapper mode.

This page was last checked against public project pages on June 10, 2026.

## Positioning

| Project | Public positioning | Main surface | Context Firewall difference |
| --- | --- | --- | --- |
| RTK | CLI proxy for compressing command outputs before they reach the LLM. Public pages claim 60-90% savings and broad command support. | Terminal command proxy and hooks, with Claude Code, Cursor, and general terminal positioning. | Context Firewall stores raw evidence locally, exposes `cfw show`, emits receipts, and refuses hook-native Codex savings until a canary proves replacement. |
| Headroom | Compresses tool outputs, logs, RAG chunks, files, and conversation history. Offers library, proxy, agent wrap, MCP server, shared memory, and learning flows. | Broad compression layer across apps and agents. | Context Firewall avoids LLM compression in core and keeps the first product surface small: deterministic reducers, local artifacts, policy gates, and proof-based receipts. |
| Context Mode | Context-window optimization for coding agents, with output sandboxing, MCP tools, hooks, and many platform integrations. Public pages claim 98% reduction and 15 platforms. | MCP/server/plugin and platform-specific hook routing. | Context Firewall starts with Codex-only real behavior, keeps hook-native blocked when unproven, and treats wrapper mode as advisory rather than pretending full interception. |

## Design Tradeoffs

Context Firewall optimizes for trust over maximum headline savings:

- Raw evidence stays queryable with span handles.
- Reducers are deterministic and easy to audit.
- Policy can block waste before command execution.
- Duplicate suppression requires a repeat fingerprint, not a vague "same task" guess.
- Savings claims are tied to delivery status.

This means Context Firewall may report lower savings than tools that compress more aggressively or route more surfaces. That is acceptable. The core promise is that a receipt should survive scrutiny.

## Current Limits

- Codex hook-native mode is not enabled because the current real canary is negative.
- Wrapper mode depends on the agent following explicit `cfw run -- ...` routing instructions.
- Context Firewall does not yet manage conversation history, RAG chunks, MCP schemas, or cross-agent memory.
- Claude Code, Gemini CLI, Cursor, OpenClaw, and MCP adapters are planned after the Codex path is proven.

## Sources

- RTK GitHub: <https://github.com/rtk-ai/rtk>
- RTK site: <https://www.rtk-ai.app/>
- Headroom GitHub: <https://github.com/chopratejas/headroom>
- Context Mode GitHub: <https://github.com/mksglu/context-mode>
- Context Mode architecture discussion: <https://news.ycombinator.com/item?id=47193064>

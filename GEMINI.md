<!-- context-firewall:start -->
## Context Firewall

Use Context Firewall for noisy terminal output.

- Prefer `cfw run -- <command>` for tests, builds, git diffs, search results,
  file dumps, generated files, browser snapshots, and logs.
- If MCP tools are available, prefer `cfw_run`, `cfw_show`, `cfw_spans`, and
  `cfw_receipt`.
- When Context Firewall returns a span handle like `cfw://span/<id>`, retrieve
  exact lines with `cfw show <id> --lines A:B` instead of rerunning the command.

Keep raw evidence on disk. Keep agent context clean.
<!-- context-firewall:end -->

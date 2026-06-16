# Context Firewall implementation plan

This plan is the execution contract for the next Context Firewall work. Keep the
product boundary narrow: evidence-preserving command and tool output for coding
agents. Do not turn this into a general LLM proxy, memory platform, or broad
context manager.

## Execution rules

- Use real commands, real ledger data, and real local artifacts.
- Use Context Firewall for noisy checks and keep span handles in PR notes.
- Ship one coherent block per PR.
- Each PR must include the command output or CFW span proving validation.
- No mocks, demos, silent fallbacks, or fake success paths.
- Prefer deterministic reducers and local data over LLM-based decisions.

## Non-goals

- LLM API proxying.
- Provider cache alignment.
- Cross-agent memory.
- Hosted telemetry.
- Semantic or ML compression in the core path.

## Block 1: analytics commands

Goal: make the value visible from the existing SQLite ledger.

Add:

- `cfw gain`
- `cfw discover`
- `cfw session`

Behavior:

- `cfw gain` reports raw tokens, returned tokens, saved tokens, reduction
  percentage, and command count.
- `cfw discover` reports commands with low savings, repeated passthrough, large
  raw output, and repeated unchanged output.
- `cfw session` reports recent adoption from stored spans: CFW-routed commands,
  reducer mix, delivery status mix, and top noisy commands.

Acceptance:

- Commands read from the existing store only.
- Empty ledgers print useful zero-state output.
- JSON output is not required in the first pass.
- Existing `cfw receipt`, `cfw spans`, and `cfw top` behavior stays intact.

Validation:

- `cargo fmt --check`
- `cargo test -p cfw-store -p cfw-cli`
- A real `cfw run -- <command>` followed by `cfw gain`, `cfw discover`, and
  `cfw session`.

## Block 2: queryable retrieval

Goal: let agents find raw evidence without knowing line numbers first.

Add:

- `cfw show <span> --grep <pattern>`
- `cfw show <span> --around <n>`
- `cfw show <span> --json-path <path>`
- `cfw search-spans <pattern>`

Behavior:

- `--grep` searches the stored raw artifact.
- `--around` includes surrounding lines for each match.
- `--json-path` extracts a JSON value from stored raw output.
- `search-spans` searches recent artifacts and prints span id, command, line
  number, and matching line.
- Secret-like guard behavior must remain at least as strict as current `show`.

Acceptance:

- Exact line retrieval keeps working.
- Missing span and missing artifact errors stay explicit.
- Pattern matching can be plain substring first; regex is not required.

Validation:

- `cargo fmt --check`
- `cargo test -p cfw-cli`
- Create a real span containing a known marker and retrieve it with `--grep`.

## Block 3: reducer DSL

Goal: let users add simple line filters without writing Rust.

Add:

- Project config: `.cfw/reducers.toml`
- User config: a global reducers file under the existing CFW config directory
- Fields: `match_command`, `strip_lines_matching`, `keep_lines_matching`,
  `max_lines`, `tail_lines`, `on_empty`

Behavior:

- First matching reducer wins.
- Project reducer overrides user reducer.
- Built-in Rust reducers still win for explicit `--kind`.
- The DSL only handles line filtering and truncation.

Acceptance:

- Invalid TOML fails clearly.
- Unknown fields fail clearly.
- Regex compilation errors identify the reducer name.
- No plugin system, dynamic code, network access, or shell execution.

Validation:

- `cargo fmt --check`
- `cargo test -p cfw-reducers -p cfw-cli`
- Real local `.cfw/reducers.toml` smoke test with a command that produces
  removable noise.

## Block 4: higher-value built-in reducers

Goal: cover the command outputs that burn agent context most often.

Add focused reducers or DSL-backed built-ins for:

- `cargo test`
- `pytest`
- `npm test`, `vitest`, `jest`
- `tsc`
- `eslint`
- `docker logs`
- `kubectl logs`
- `gh pr view`, `gh pr checks`
- `terraform plan`

Behavior:

- Keep failures, summaries, file paths, line numbers, and actionable diagnostics.
- Drop progress bars, repeated pass lines, repeated log noise, and large
  unchanged sections.
- Prefer small deterministic heuristics over broad parsers.

Acceptance:

- Each reducer has one representative real-output fixture or generated command
  output captured from a real tool where available.
- Reducers never hide non-zero exit status.
- When uncertain, keep more evidence and rely on `cfw show`.

Validation:

- `cargo fmt --check`
- `cargo test -p cfw-reducers -p cfw-cli`
- At least one real command per installed ecosystem available locally.

## Block 5: codex auto-rewrite adapter

Goal: reduce dependence on agent discipline.

Add:

- `cfw install codex` should install the existing AGENTS.md guidance and, where
  Codex exposes a real hook path, install a command rewrite adapter.
- Rewrite examples:
  - `git diff` -> `cfw run --kind git -- git diff`
  - `cargo test` -> `cfw run --kind test-output -- cargo test`
  - `rg ...` -> `cfw run --kind search -- rg ...`

Behavior:

- If no real Codex hook path exists, fail explicitly and keep the guidance-only
  install path.
- Do not pretend auto-rewrite is active when only AGENTS.md was written.
- Adapter install and uninstall must be idempotent.

Acceptance:

- `cfw doctor codex` reports guidance status and auto-rewrite status separately.
- `cfw uninstall codex` removes only CFW-managed files or managed blocks.
- No destructive edits to user settings without backup or explicit supported
  install path.

Validation:

- `cargo fmt --check`
- `cargo test -p cfw-codex -p cfw-cli`
- Real install/uninstall dry run or isolated temp-home integration test.

## Block 6: MCP expansion

Goal: expose analytics and queryable retrieval without shelling out.

Add MCP tools:

- `cfw_search`
- `cfw_gain`
- `cfw_discover`
- `cfw_session`
- Optional: `cfw_reduce`

Behavior:

- MCP output should be compact by default.
- Raw content should remain on disk and be retrieved by span or exact query.
- Tool schemas must match CLI behavior.

Acceptance:

- Existing MCP tools continue working.
- CLI and MCP share implementation where practical.
- No duplicate business logic if a small shared function is enough.

Validation:

- `cargo fmt --check`
- `cargo test`
- Real MCP smoke test if local host tooling is available.

## Block 7: learn from misses

Goal: turn repeated failures and reducer misses into actionable local rules.

Add:

- `cfw learn`

Behavior:

- Analyze local ledger only.
- Detect repeated failed commands, repeated `cfw show` lookups after a reducer,
  low-savings reducers, and repeated large commands without a matching reducer.
- Print suggestions for `AGENTS.md` and `.cfw/reducers.toml`.
- Do not edit files by default.

Acceptance:

- `cfw learn` is read-only unless an explicit apply flag is added later.
- Suggestions include evidence: span ids, commands, reducer names, counts.
- No LLM call in the first version.

Validation:

- `cargo fmt --check`
- `cargo test -p cfw-cli`
- Real ledger with at least one repeated pattern.

## Block 8: reducer quality stats

Goal: make reducer quality measurable, not just savings-heavy.

Add:

- Per-reducer savings.
- Raw fetch rate.
- Rerun rate where repeat fingerprints match.
- Failure correlation by exit code.

Behavior:

- Store only aggregate local stats.
- Use existing spans where possible before adding schema.
- Prefer `cfw gain --reducers` or `cfw session --reducers` before a new command.

Acceptance:

- A reducer with high savings but high raw-fetch rate is visible.
- No hosted telemetry.
- No path or command upload.

Validation:

- `cargo fmt --check`
- `cargo test -p cfw-store -p cfw-cli`
- Real run that creates spans for at least two reducers.

## Finish audit

Before calling the goal done:

- Every block above is either merged or explicitly removed from the plan.
- The final tree is clean.
- `cargo fmt --check` passes.
- `cargo test` passes.
- `scripts/release-smoke.sh` passes.
- `cfw doctor codex` accurately reports guidance and auto-rewrite state.
- `cfw receipt`, `cfw gain`, `cfw discover`, and `cfw session` work on a real
  local ledger.

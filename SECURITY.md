# Security and Privacy

Context Firewall handles raw command output. That output can include private source code, credentials, logs, customer data, and local paths. The project is designed around a strict local-first trust boundary.

## Guarantees

- Raw stdout and stderr are stored on the local machine.
- Core reducers are deterministic and do not call an LLM.
- Context Firewall does not send telemetry.
- `cfw show` blocks secret-like raw output unless the user passes `--force`.
- `cfw purge` deletes local span rows and artifact files from the active data directory.
- Receipts only count savings from delivery states that prove compact output was returned to the agent.

## Data Stored Locally

Context Firewall stores:

- span metadata in a local SQLite database.
- raw combined stdout/stderr artifacts.
- split stdout and stderr artifacts.
- reducer metadata, repeat fingerprints, and receipt inputs.

Run this to inspect the active data directory:

```bash
cfw first-run
cfw spans
```

Run this to delete local evidence:

```bash
cfw purge --older-than-days 14
cfw purge --all
```

## Threat Model

Context Firewall is meant to reduce accidental context leakage and token waste between local tools and coding agents. It is not a sandbox, malware scanner, or secrets manager.

In scope:

- oversized command output entering an agent context.
- accidental raw output retrieval.
- false savings claims.
- noisy repeated output.
- policy checks for generated, dependency, denied, and binary paths.

Out of scope:

- protecting against malicious commands the user intentionally runs.
- enforcing OS-level process isolation.
- preventing another local process from reading files the user can read.
- guaranteeing that third-party agents honor advisory wrapper instructions.

## Reporting Vulnerabilities

Please open a private security advisory on GitHub if the repository is public and advisories are enabled. If not, open an issue with a minimal description and omit exploit details until a private channel is available.

Do not include secrets, private code, or raw customer data in reports.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use cfw_core::ids::new_id;
use cfw_core::receipt::{RECEIPT_SCHEMA_JSON, RECEIPT_SCHEMA_VERSION};
use cfw_core::span::{DeliveryStatus, SpanRecord};
use cfw_core::token::estimate_tokens;
use cfw_policy::{Policy, PolicyAction};
use cfw_store::paths::StorePaths;
use cfw_store::sqlite::Store;
use chrono::{Duration, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use regex::Regex;

const POLICY_ENGINE_VERSION: &str = "cfw-policy-v1";
const REPEAT_FINGERPRINT_SCHEMA_VERSION: &str = "cfw.repeat_fingerprint.v1";
const ENV_REPEAT_ALLOWLIST: &[&str] = &[
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "PATH",
    "NODE_ENV",
    "RUSTFLAGS",
    "RUSTUP_TOOLCHAIN",
    "CARGO_HOME",
    "RUSTC_WRAPPER",
    "PYTHONPATH",
    "VIRTUAL_ENV",
    "CONDA_PREFIX",
];

#[derive(Debug, Parser)]
#[command(
    name = "cfw",
    version,
    about = "Local-first context firewall for coding agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Install an explicit agent adapter.
    Install(InstallArgs),
    /// Remove an explicit agent adapter.
    Uninstall(UninstallArgs),
    /// Run a guided local command to prove the storage and receipt path works.
    FirstRun,
    /// Run a real command, store raw output, and print compact output.
    Run(RunArgs),
    /// Compact stdin with a deterministic reducer.
    Compact(CompactArgs),
    /// Show raw artifact output for a span.
    Show(ShowArgs),
    /// List recent spans from the local ledger.
    Spans(SpansArgs),
    /// Print a local receipt from recent spans.
    Receipt(ReceiptArgs),
    /// Delete local span rows and artifacts.
    Purge(PurgeArgs),
    /// Manage and inspect Context Firewall policy.
    Policy(PolicyArgs),
    /// Show the largest recent context burners.
    Top(TopArgs),
    /// Check local Context Firewall and Codex integration health.
    Doctor(DoctorArgs),
    /// Run real integration canaries.
    Canary(CanaryArgs),
}

#[derive(Debug, Args)]
struct InstallArgs {
    /// Adapter target to install.
    target: String,

    /// Adapter mode.
    #[arg(long, value_enum, default_value_t = InstallMode::Wrapper)]
    mode: InstallMode,

    /// Write the managed AGENTS.md block instead of printing it.
    #[arg(long)]
    write_agents: bool,

    /// Print the planned write without modifying files.
    #[arg(long)]
    dry_run: bool,

    /// Path to AGENTS.md when --write-agents is set.
    #[arg(long, default_value = "AGENTS.md")]
    agents_path: PathBuf,
}

#[derive(Debug, Args)]
struct UninstallArgs {
    /// Adapter target to uninstall.
    target: String,

    /// Path to AGENTS.md containing the managed block.
    #[arg(long, default_value = "AGENTS.md")]
    agents_path: PathBuf,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InstallMode {
    Wrapper,
    HookNative,
}

#[derive(Debug, Args)]
struct RunArgs {
    /// Reducer kind to apply to captured command output.
    #[arg(long, default_value = "generic")]
    kind: String,

    /// Read this file and pass its exact bytes to the command's stdin.
    #[arg(long, value_name = "PATH")]
    stdin_file: Option<PathBuf>,

    /// Command and arguments to execute.
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[derive(Debug)]
struct StdinEvidence {
    path: PathBuf,
    bytes: usize,
    hash: String,
}

#[derive(Debug, Args)]
struct CompactArgs {
    /// Reducer kind to apply to stdin.
    #[arg(long, default_value = "generic")]
    kind: String,
}

#[derive(Debug, Args)]
struct ShowArgs {
    /// Span id to retrieve.
    span_id: String,

    /// Optional 1-based inclusive line range, formatted A:B.
    #[arg(long)]
    lines: Option<String>,

    /// Bypass secret-like output guard.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct SpansArgs {
    /// Number of spans to show.
    #[arg(long, default_value_t = 20)]
    limit: i64,

    /// Emit JSON instead of terminal text.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PurgeArgs {
    /// Delete every local span and artifact in this Context Firewall data dir.
    #[arg(long)]
    all: bool,

    /// Delete spans older than this many days.
    #[arg(long)]
    older_than_days: Option<i64>,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    /// Include Codex-specific checks.
    #[arg(value_name = "TARGET")]
    target: Option<String>,
}

#[derive(Debug, Args)]
struct CanaryArgs {
    /// Canary target to run.
    #[arg(value_name = "TARGET")]
    target: String,

    /// Codex binary to execute.
    #[arg(long, default_value = "codex")]
    codex_bin: String,

    /// Optional model override for the real Codex exec run.
    #[arg(long)]
    model: Option<String>,
}

#[derive(Debug, Args)]
struct ReceiptArgs {
    /// Emit JSON instead of terminal text.
    #[arg(long)]
    json: bool,

    /// Print the JSON Schema for `cfw receipt --json`.
    #[arg(long)]
    schema: bool,
}

#[derive(Debug, Args)]
struct TopArgs {
    /// Number of spans to show.
    #[arg(long, default_value_t = 10)]
    limit: i64,
}

#[derive(Debug, Args)]
struct PolicyArgs {
    #[command(subcommand)]
    command: PolicyCommand,
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    /// Create the default policy file if one does not exist.
    Init,
    /// Parse and validate the current policy file.
    Check,
    /// Explain how policy classifies a command.
    Explain {
        /// Command and arguments to classify.
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Install(args) => install(args),
        Commands::Uninstall(args) => uninstall(args),
        Commands::FirstRun => first_run(),
        Commands::Run(args) => run_command(args),
        Commands::Compact(args) => compact(args),
        Commands::Show(args) => show(args),
        Commands::Spans(args) => spans(args),
        Commands::Receipt(args) => receipt(args),
        Commands::Purge(args) => purge(args),
        Commands::Policy(args) => policy(args),
        Commands::Top(args) => top(args),
        Commands::Doctor(args) => doctor(args),
        Commands::Canary(args) => canary(args),
    }
}

fn first_run() -> Result<()> {
    eprintln!("Context Firewall first run: executing a real local command through cfw run.");
    run_command(RunArgs {
        kind: "test-output".to_string(),
        stdin_file: None,
        command: vec![
            "sh".to_string(),
            "-c".to_string(),
            "printf 'running 2 tests\\ntest smoke ... ok\\ntest context_firewall_demo ... ok\\ntest result: ok. 2 passed; 0 failed\\n'"
                .to_string(),
        ],
    })
}

fn install(args: InstallArgs) -> Result<()> {
    if args.target != "codex" {
        bail!(
            "unsupported adapter `{}`; only `codex` is available",
            args.target
        );
    }

    match args.mode {
        InstallMode::HookNative => {
            bail!(
                "HookReplacementFailed: hook-native install is blocked until the Codex output-replacement canary passes. Use `cfw install codex --mode wrapper`."
            );
        }
        InstallMode::Wrapper => {
            println!("Context Firewall Codex adapter");
            println!("  mode: wrapper");
            println!("  enforcement: advisory");
            println!("  hook_replacement_verified: false");
            if args.write_agents {
                let outcome = if args.dry_run {
                    cfw_codex::install::inspect_wrapper_snippet(&args.agents_path)?
                } else {
                    cfw_codex::install::write_wrapper_snippet(&args.agents_path)?
                };
                println!("  agents_path: {}", args.agents_path.display());
                println!("  dry_run: {}", args.dry_run);
                println!("  result: {:?}", outcome);
                if args.dry_run {
                    println!();
                    println!("{}", cfw_codex::install::wrapper_snippet());
                }
            } else {
                println!();
                println!("{}", cfw_codex::install::wrapper_snippet());
            }
        }
    }
    Ok(())
}

fn uninstall(args: UninstallArgs) -> Result<()> {
    if args.target != "codex" {
        bail!(
            "unsupported adapter `{}`; only `codex` is available",
            args.target
        );
    }

    let outcome = cfw_codex::install::uninstall_wrapper_snippet(&args.agents_path)?;
    println!("Context Firewall Codex adapter");
    println!("  mode: wrapper");
    println!("  agents_path: {}", args.agents_path.display());
    println!("  result: {:?}", outcome);
    Ok(())
}

fn run_command(args: RunArgs) -> Result<()> {
    let Some((program, rest)) = args.command.split_first() else {
        bail!("CfwExecutionError: missing command");
    };
    let command_text = args.command.join(" ");

    let paths = StorePaths::discover()?;
    let policy = load_or_default_policy(&paths)?;
    let cwd = std::env::current_dir().context("CfwExecutionError: could not read cwd")?;
    let decision = policy.decide_command(&args.command, &cwd);
    match decision.action {
        PolicyAction::Block => {
            bail!(
                "PolicyBlocked: {} ({})",
                decision.explanation,
                decision.reason_code
            );
        }
        PolicyAction::Ask => {
            bail!(
                "PolicyAskRequired: noninteractive `cfw run` cannot ask for approval; command was not executed ({})",
                decision.reason_code
            );
        }
        _ => {}
    }

    let (stdin_evidence, stdin_bytes) = match read_stdin_payload(args.stdin_file.as_deref())? {
        Some((evidence, bytes)) => (Some(evidence), Some(bytes)),
        None => (None, None),
    };
    let mut command = Command::new(program);
    command
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin_bytes.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("CfwExecutionError: could not run `{command_text}`"))?;
    let stdin_writer = if let Some(bytes) = stdin_bytes {
        let Some(mut stdin) = child.stdin.take() else {
            bail!("CfwExecutionError: stdin pipe was not available for `{command_text}`");
        };
        let path = stdin_evidence
            .as_ref()
            .expect("stdin evidence exists when stdin bytes exist")
            .path
            .clone();
        Some(std::thread::spawn(move || -> Result<()> {
            match stdin.write_all(&bytes) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "CfwExecutionError: could not write stdin from {}",
                            path.display()
                        )
                    });
                }
            }
            Ok(())
        }))
    } else {
        None
    };
    let output = child.wait_with_output().with_context(|| {
        format!("CfwExecutionError: could not collect output for `{command_text}`")
    })?;
    if let Some(writer) = stdin_writer {
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("CfwExecutionError: stdin writer thread panicked"))??;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n[stderr]\n{stderr}")
    };
    let reducer_kind = choose_reducer_kind(&args.kind, decision.reason_code);
    let mut reduction = cfw_reducers::reduce(reducer_kind, &raw);
    let span_kind = reducer_kind.to_string();
    let raw_estimate = estimate_tokens(&raw);

    let store = Store::open(&paths)?;
    let session_id = current_session_id();
    store.ensure_session(
        &session_id,
        "cfw",
        Some(&cwd.display().to_string()),
        None,
        Some(env!("CARGO_PKG_VERSION")),
    )?;
    let span_id = new_id();
    let session_dir = paths.sessions_dir.join(&session_id).join("artifacts");
    std::fs::create_dir_all(&session_dir)
        .with_context(|| format!("could not create {}", session_dir.display()))?;
    let artifact_path = session_dir.join(format!("{span_id}.txt"));
    let stdout_path = session_dir.join(format!("{span_id}.stdout"));
    let stderr_path = session_dir.join(format!("{span_id}.stderr"));
    let meta_path = session_dir.join(format!("{span_id}.meta.json"));
    std::fs::write(&artifact_path, raw.as_bytes())
        .with_context(|| format!("could not write {}", artifact_path.display()))?;
    std::fs::write(&stdout_path, output.stdout.as_slice())
        .with_context(|| format!("could not write {}", stdout_path.display()))?;
    std::fs::write(&stderr_path, output.stderr.as_slice())
        .with_context(|| format!("could not write {}", stderr_path.display()))?;
    let hash = blake3::hash(raw.as_bytes()).to_hex().to_string();
    let repeat_fingerprint = repeat_fingerprint(
        &args.command,
        &cwd,
        output.status.code(),
        &hash,
        &paths,
        stdin_evidence.as_ref(),
    )?;
    let repeat_key = hash_json_value(&repeat_fingerprint);

    let meta = serde_json::json!({
        "id": span_id,
        "session_id": session_id,
        "command": command_text,
        "cwd": cwd.display().to_string(),
        "exit_code": output.status.code(),
        "argv": args.command,
        "stdout_path": stdout_path.display().to_string(),
        "stderr_path": stderr_path.display().to_string(),
        "combined_path": artifact_path.display().to_string(),
        "stdin": stdin_evidence.as_ref().map(stdin_evidence_json),
        "repeat_key": repeat_key.clone(),
        "repeat_fingerprint": repeat_fingerprint,
    });
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)
        .with_context(|| format!("could not write {}", meta_path.display()))?;

    let duplicate = store.find_duplicate_span_by_repeat_key(&repeat_key)?;
    let mut deduped = false;
    if let Some(previous) = duplicate.as_ref() {
        let duplicate_output = format!(
            "[context-firewall: duplicate output]\nprevious_span: cfw://span/{}\nproof: same repeat fingerprint: command, cwd, exit code, raw output hash, git HEAD, git index, selected env hash, policy version, and known input file hashes\nraw output stored for this run; use cfw show {} for the previous copy\n",
            previous.id, previous.id
        );
        if estimate_tokens(&duplicate_output).tokens < estimate_tokens(&reduction.output).tokens {
            reduction.output = duplicate_output;
            reduction.omitted = true;
            reduction
                .notes
                .push(format!("deduped against previous span {}", previous.id));
            deduped = true;
        }
    }
    let returned_estimate = estimate_tokens(&reduction.output);
    let span = SpanRecord {
        id: span_id.clone(),
        session_id,
        kind: span_kind,
        source: "cfw_run".to_string(),
        command: Some(command_text),
        cwd: Some(cwd.display().to_string()),
        exit_code: output.status.code(),
        raw_bytes: raw.len() as i64,
        raw_estimated_tokens: raw_estimate.tokens,
        returned_bytes: reduction.output.len() as i64,
        returned_estimated_tokens: returned_estimate.tokens,
        hash,
        reducer: Some(reduction.reducer),
        policy_action: decision.action.as_str().to_string(),
        delivery_status: DeliveryStatus::AdvisoryWrapper,
        delivery_evidence_path: None,
        repeat_key,
        repeat_evidence_json: serde_json::to_string(&repeat_fingerprint)?,
        risk_class: if deduped {
            "deduped"
        } else if reduction.omitted {
            "reduced"
        } else {
            "pass_through"
        }
        .to_string(),
        artifact_path: artifact_path.display().to_string(),
        created_at: Utc::now(),
    };
    store.insert_span(&span)?;

    print!("{}", reduction.output);
    println!("\n[context-firewall]");
    println!("span: cfw://span/{span_id}");
    println!(
        "raw: {} bytes, estimated {} tokens",
        span.raw_bytes, span.raw_estimated_tokens
    );
    println!(
        "returned: {} bytes, estimated {} tokens",
        span.returned_bytes, span.returned_estimated_tokens
    );
    println!("delivery_status: {}", span.delivery_status.as_str());
    println!("full output stored locally");
    println!("commands:");
    println!("  cfw show {span_id}");
    println!("  cfw show {span_id} --lines 120:180");
    println!("[/context-firewall]");

    std::process::exit(output.status.code().unwrap_or(1));
}

fn compact(args: CompactArgs) -> Result<()> {
    let input = std::io::read_to_string(std::io::stdin()).context("could not read stdin")?;
    let reduction = cfw_reducers::reduce(&args.kind, &input);
    print!("{}", reduction.output);
    Ok(())
}

fn show(args: ShowArgs) -> Result<()> {
    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    let Some(span) = store.get_span(&args.span_id)? else {
        bail!("span not found: {}", args.span_id);
    };
    let artifact = std::fs::read_to_string(&span.artifact_path)
        .with_context(|| format!("could not read {}", span.artifact_path))?;

    if let Some(range) = args.lines {
        let (start, end) = parse_line_range(&range)?;
        let mut selected = String::new();
        for (idx, line) in artifact.lines().enumerate() {
            let line_no = idx + 1;
            if line_no >= start && line_no <= end {
                selected.push_str(&format!("{line_no}: {line}\n"));
            }
        }
        guard_secret_like_output(&selected, args.force)?;
        print!("{selected}");
    } else {
        guard_secret_like_output(&artifact, args.force)?;
        print!("{artifact}");
    }
    Ok(())
}

fn spans(args: SpansArgs) -> Result<()> {
    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    let spans = store.recent_spans(args.limit)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&spans)?);
        return Ok(());
    }

    println!("Context Firewall Spans");
    println!();
    for span in spans {
        println!(
            "{} {} raw={} returned={} delivery={} created={}",
            &span.id[..12],
            span.kind,
            span.raw_estimated_tokens,
            span.returned_estimated_tokens,
            span.delivery_status.as_str(),
            span.created_at.to_rfc3339()
        );
        if let Some(command) = span.command {
            println!("   command: {command}");
        }
    }
    Ok(())
}

fn receipt(args: ReceiptArgs) -> Result<()> {
    if args.schema {
        println!("{RECEIPT_SCHEMA_JSON}");
        return Ok(());
    }

    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    let spans = store.recent_spans(50)?;

    let raw: i64 = spans.iter().map(|span| span.raw_estimated_tokens).sum();
    let returned: i64 = spans
        .iter()
        .map(|span| span.returned_estimated_tokens)
        .sum();
    let verified_saved: i64 = spans
        .iter()
        .filter(|span| {
            matches!(
                span.delivery_status,
                DeliveryStatus::AdvisoryWrapper | DeliveryStatus::ReplacedToolResult
            )
        })
        .map(|span| span.raw_estimated_tokens - span.returned_estimated_tokens)
        .sum();

    if args.json {
        let payload = serde_json::json!({
            "schema_version": RECEIPT_SCHEMA_VERSION,
            "spans": spans.len(),
            "raw_estimated_tokens": raw,
            "returned_estimated_tokens": returned,
            "net_estimated_saved": verified_saved.max(0),
            "confidence": "low",
            "recent_spans": spans.iter().take(10).map(|span| {
                serde_json::json!({
                    "id": span.id,
                    "kind": span.kind,
                    "raw_estimated_tokens": span.raw_estimated_tokens,
                    "returned_estimated_tokens": span.returned_estimated_tokens,
                    "delivery_status": span.delivery_status.as_str(),
                    "command": span.command,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("Context Firewall Receipt");
    println!();
    println!("Observed:");
    println!("  spans: {}", spans.len());
    println!("  raw estimated tokens: {raw}");
    println!("  returned estimated tokens: {returned}");
    println!();
    println!("Estimated:");
    println!("  net saved: {}", verified_saved.max(0));
    println!("  confidence: low");
    println!();
    println!("Recent spans:");
    for span in spans.iter().take(10) {
        println!(
            "  {} {} {} -> {} tokens ({})",
            &span.id[..12],
            span.kind,
            span.raw_estimated_tokens,
            span.returned_estimated_tokens,
            span.delivery_status.as_str()
        );
    }
    Ok(())
}

fn policy(args: PolicyArgs) -> Result<()> {
    let paths = StorePaths::discover()?;
    let policy_path = paths.data_dir.join("config.toml");
    match args.command {
        PolicyCommand::Init => {
            let written = Policy::write_default(&policy_path)?;
            if written {
                println!("created policy: {}", policy_path.display());
            } else {
                println!("policy already exists: {}", policy_path.display());
            }
        }
        PolicyCommand::Check => {
            let policy = load_or_default_policy(&paths)?;
            println!("policy: ok");
            println!(
                "  session_estimated_tokens: {}",
                policy.budgets.session_estimated_tokens
            );
            println!(
                "  tool_output_estimated_tokens: {}",
                policy.budgets.tool_output_estimated_tokens
            );
        }
        PolicyCommand::Explain { command } => {
            let policy = load_or_default_policy(&paths)?;
            let cwd = std::env::current_dir().context("could not read cwd")?;
            let decision = policy.decide_command(&command, &cwd);
            println!("action: {}", decision.action.as_str());
            println!("reason: {}", decision.reason_code);
            println!("explanation: {}", decision.explanation);
        }
    }
    Ok(())
}

fn top(args: TopArgs) -> Result<()> {
    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    let mut spans = store.recent_spans(200)?;
    spans.sort_by_key(|span| std::cmp::Reverse(span.raw_estimated_tokens));

    println!("Context Firewall Top Burners");
    println!();
    for (idx, span) in spans.iter().take(args.limit as usize).enumerate() {
        println!(
            "{}. {} {} raw={} returned={} delivery={}",
            idx + 1,
            &span.id[..12],
            span.kind,
            span.raw_estimated_tokens,
            span.returned_estimated_tokens,
            span.delivery_status.as_str()
        );
        if let Some(command) = &span.command {
            println!("   command: {command}");
        }
    }
    Ok(())
}

fn purge(args: PurgeArgs) -> Result<()> {
    if args.all == args.older_than_days.is_some() {
        bail!("PurgeRequiresScope: pass exactly one of --all or --older-than-days <days>");
    }

    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    let spans = if args.all {
        store.all_spans()?
    } else {
        let days = args.older_than_days.expect("checked above");
        if days < 0 {
            bail!("PurgeRequiresScope: --older-than-days must be >= 0");
        }
        let cutoff = Utc::now() - Duration::days(days);
        store.spans_before(cutoff)?
    };

    let ids = spans.iter().map(|span| span.id.clone()).collect::<Vec<_>>();
    let rows_deleted = store.delete_spans(&ids)?;
    let mut files_deleted = 0usize;
    for span in &spans {
        files_deleted += remove_span_artifacts(span)?;
    }

    println!("purged spans: {rows_deleted}");
    println!("purged artifact files: {files_deleted}");
    Ok(())
}

fn load_or_default_policy(paths: &StorePaths) -> Result<Policy> {
    let policy_path = paths.data_dir.join("config.toml");
    if policy_path.exists() {
        Policy::load(&policy_path)
    } else {
        Ok(Policy::default())
    }
}

fn repeat_fingerprint(
    argv: &[String],
    cwd: &Path,
    exit_code: Option<i32>,
    raw_output_hash: &str,
    paths: &StorePaths,
    stdin_evidence: Option<&StdinEvidence>,
) -> Result<serde_json::Value> {
    let (env_hash, env_present) = selected_env_hash();
    Ok(serde_json::json!({
        "schema_version": REPEAT_FINGERPRINT_SCHEMA_VERSION,
        "command": argv.join(" "),
        "argv": argv,
        "cwd": cwd.display().to_string(),
        "exit_code": exit_code,
        "raw_output_hash": raw_output_hash,
        "git_head": git_output(cwd, &["rev-parse", "--verify", "HEAD"]),
        "git_index_hash": git_output(cwd, &["write-tree"]),
        "selected_env": {
            "allowlist": ENV_REPEAT_ALLOWLIST,
            "present": env_present,
            "hash": env_hash,
        },
        "policy": {
            "engine_version": POLICY_ENGINE_VERSION,
            "config_hash": policy_config_hash(paths)?,
        },
        "input_files": input_file_hashes(argv, cwd)?,
        "dependencies": dependency_fingerprints(argv, cwd)?,
        "stdin": stdin_evidence.map(stdin_evidence_json),
    }))
}

fn read_stdin_payload(path: Option<&Path>) -> Result<Option<(StdinEvidence, Vec<u8>)>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let canonical = path.canonicalize().with_context(|| {
        format!(
            "CfwExecutionError: could not canonicalize stdin file {}",
            path.display()
        )
    })?;
    if !canonical.is_file() {
        bail!(
            "CfwExecutionError: stdin file is not a regular file: {}",
            canonical.display()
        );
    }
    let bytes = std::fs::read(&canonical).with_context(|| {
        format!(
            "CfwExecutionError: could not read stdin file {}",
            canonical.display()
        )
    })?;
    let evidence = StdinEvidence {
        path: canonical,
        bytes: bytes.len(),
        hash: blake3::hash(&bytes).to_hex().to_string(),
    };
    Ok(Some((evidence, bytes)))
}

fn stdin_evidence_json(evidence: &StdinEvidence) -> serde_json::Value {
    serde_json::json!({
        "source": "file",
        "path": evidence.path.display().to_string(),
        "bytes": evidence.bytes,
        "hash": evidence.hash,
    })
}

fn hash_json_value(value: &serde_json::Value) -> String {
    blake3::hash(value.to_string().as_bytes())
        .to_hex()
        .to_string()
}

fn selected_env_hash() -> (String, Vec<&'static str>) {
    let mut hasher = blake3::Hasher::new();
    let mut present = Vec::new();
    for name in ENV_REPEAT_ALLOWLIST {
        if let Ok(value) = std::env::var(name) {
            present.push(*name);
            hasher.update(name.as_bytes());
            hasher.update(b"\0");
            hasher.update(value.as_bytes());
            hasher.update(b"\0");
        }
    }
    (hasher.finalize().to_hex().to_string(), present)
}

fn policy_config_hash(paths: &StorePaths) -> Result<Option<String>> {
    let path = paths.data_dir.join("config.toml");
    if !path.exists() {
        return Ok(None);
    }
    let bytes =
        std::fs::read(&path).with_context(|| format!("could not read {}", path.display()))?;
    Ok(Some(blake3::hash(&bytes).to_hex().to_string()))
}

fn input_file_hashes(argv: &[String], cwd: &Path) -> Result<Vec<serde_json::Value>> {
    let mut files = Vec::new();
    for (idx, arg) in argv.iter().enumerate().skip(1) {
        if arg.starts_with('-') {
            continue;
        }
        let candidate = cwd.join(arg);
        if !candidate.is_file() {
            continue;
        }
        let canonical = candidate
            .canonicalize()
            .with_context(|| format!("could not canonicalize {}", candidate.display()))?;
        let bytes = std::fs::read(&canonical)
            .with_context(|| format!("could not read {}", canonical.display()))?;
        files.push(serde_json::json!({
            "argument_index": idx,
            "argument": arg,
            "path": canonical.display().to_string(),
            "bytes": bytes.len(),
            "hash": blake3::hash(&bytes).to_hex().to_string(),
        }));
    }
    Ok(files)
}

fn dependency_fingerprints(argv: &[String], cwd: &Path) -> Result<Vec<serde_json::Value>> {
    let families = command_dependency_families(argv);
    if families.is_empty() {
        return Ok(Vec::new());
    }

    let mut fingerprints = Vec::new();
    for family in families {
        let roots = dependency_roots_for_family(&family, argv, cwd);
        let files = dependency_files_for_family(&family, &roots)?;
        if files.is_empty() {
            continue;
        }
        fingerprints.push(serde_json::json!({
            "family": family.as_str(),
            "roots": roots.iter().map(|root| root.display().to_string()).collect::<Vec<_>>(),
            "files": files,
        }));
    }
    Ok(fingerprints)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DependencyFamily {
    Cargo,
    Node,
    Python,
}

impl DependencyFamily {
    fn as_str(self) -> &'static str {
        match self {
            DependencyFamily::Cargo => "cargo",
            DependencyFamily::Node => "node",
            DependencyFamily::Python => "python",
        }
    }
}

fn command_dependency_families(argv: &[String]) -> BTreeSet<DependencyFamily> {
    let mut families = BTreeSet::new();
    let Some(command) = argv.first().map(|value| command_basename(value)) else {
        return families;
    };

    match command.as_str() {
        "cargo" | "cargo-nextest" | "nextest" => {
            families.insert(DependencyFamily::Cargo);
        }
        "npm" | "npx" | "pnpm" | "yarn" | "bun" | "node" | "jest" | "vitest" | "playwright"
        | "turbo" | "nx" => {
            families.insert(DependencyFamily::Node);
        }
        "pytest" | "py.test" | "ruff" | "mypy" | "tox" => {
            families.insert(DependencyFamily::Python);
        }
        "python" | "python3" | "uv" | "poetry" => {
            if command_invokes_pytest(argv) {
                families.insert(DependencyFamily::Python);
            }
        }
        _ => {}
    }

    families
}

fn command_invokes_pytest(argv: &[String]) -> bool {
    let Some(command) = argv.first().map(|value| command_basename(value)) else {
        return false;
    };
    match command.as_str() {
        "python" | "python3" => argv
            .windows(2)
            .any(|window| window[0] == "-m" && matches!(window[1].as_str(), "pytest" | "py.test")),
        "uv" | "poetry" => argv
            .iter()
            .skip(1)
            .map(|value| command_basename(value))
            .any(|value| matches!(value.as_str(), "pytest" | "py.test")),
        _ => false,
    }
}

fn command_basename(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(value)
        .to_ascii_lowercase()
}

fn dependency_roots_for_family(
    family: &DependencyFamily,
    argv: &[String],
    cwd: &Path,
) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    if matches!(family, DependencyFamily::Cargo)
        && let Some(manifest_parent) = cargo_manifest_parent(argv, cwd)
    {
        roots.insert(manifest_parent);
    }
    roots.insert(cwd.to_path_buf());
    roots.into_iter().collect()
}

fn cargo_manifest_parent(argv: &[String], cwd: &Path) -> Option<PathBuf> {
    let mut iter = argv.iter().skip(1).peekable();
    while let Some(arg) = iter.next() {
        let path = if arg == "--manifest-path" {
            iter.next().map(String::as_str)
        } else {
            arg.strip_prefix("--manifest-path=")
        };
        if let Some(path) = path {
            let candidate = cwd.join(path);
            return candidate.parent().map(Path::to_path_buf);
        }
    }
    None
}

fn dependency_files_for_family(
    family: &DependencyFamily,
    roots: &[PathBuf],
) -> Result<Vec<serde_json::Value>> {
    let names = match family {
        DependencyFamily::Cargo => &[
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "rust-toolchain",
            ".cargo/config.toml",
            ".cargo/config",
        ][..],
        DependencyFamily::Node => &[
            "package.json",
            "package-lock.json",
            "npm-shrinkwrap.json",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
            "yarn.lock",
            "bun.lock",
            "bun.lockb",
            "turbo.json",
            "nx.json",
        ][..],
        DependencyFamily::Python => &[
            "pyproject.toml",
            "pytest.toml",
            "pytest.ini",
            "tox.ini",
            "setup.cfg",
            "setup.py",
            "requirements.txt",
            "requirements-dev.txt",
            "requirements-test.txt",
            "uv.lock",
            "poetry.lock",
            "Pipfile.lock",
        ][..],
    };

    let mut seen = BTreeSet::new();
    let mut files = Vec::new();
    for root in roots {
        for candidate in upward_named_files(root, names) {
            let canonical = candidate
                .canonicalize()
                .with_context(|| format!("could not canonicalize {}", candidate.display()))?;
            if !seen.insert(canonical.clone()) {
                continue;
            }
            let bytes = std::fs::read(&canonical)
                .with_context(|| format!("could not read {}", canonical.display()))?;
            files.push(serde_json::json!({
                "path": canonical.display().to_string(),
                "role": dependency_file_role(&canonical),
                "bytes": bytes.len(),
                "hash": blake3::hash(&bytes).to_hex().to_string(),
            }));
        }
    }
    files.sort_by(|a, b| {
        a["path"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["path"].as_str().unwrap_or_default())
    });
    Ok(files)
}

fn upward_named_files(start: &Path, names: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut current = Some(start);
    while let Some(dir) = current {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                files.push(candidate);
            }
        }
        current = dir.parent();
    }
    files
}

fn dependency_file_role(path: &Path) -> &'static str {
    match path.file_name().and_then(OsStr::to_str).unwrap_or_default() {
        "Cargo.lock"
        | "package-lock.json"
        | "npm-shrinkwrap.json"
        | "pnpm-lock.yaml"
        | "yarn.lock"
        | "bun.lock"
        | "bun.lockb"
        | "uv.lock"
        | "poetry.lock"
        | "Pipfile.lock" => "lockfile",
        "Cargo.toml" | "package.json" | "pyproject.toml" | "setup.py" => "manifest",
        _ => "config",
    }
}

fn git_output(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn choose_reducer_kind<'a>(requested: &'a str, reason_code: &str) -> &'a str {
    if requested != "generic" {
        return requested;
    }
    match reason_code {
        "git_diff" => "git",
        "generated_file_read" => "outline",
        "json_output" => "json",
        "large_log" => "log",
        "listing_output" => "search",
        "browser_snapshot" => "browser-snapshot",
        "search_output" => "search",
        "test_output" => "test-output",
        _ => requested,
    }
}

fn doctor(args: DoctorArgs) -> Result<()> {
    let paths = StorePaths::discover()?;
    paths.ensure()?;
    println!("Context Firewall");
    println!("  data_dir: {}", paths.data_dir.display());
    println!("  db_path: {}", paths.db_path.display());
    println!("  store: ok");

    if args.target.as_deref() == Some("codex") {
        let mut codex = cfw_codex::doctor::check();
        let evidence_path = codex_canary_evidence_path(&paths);
        let verified =
            cfw_codex::canary::load_latest_verified(&evidence_path, codex.version.as_deref())?;
        if verified.is_some() {
            codex.hook_replacement_verified = true;
        }
        println!("Codex");
        println!("  found: {}", codex.codex_found);
        println!(
            "  version: {}",
            codex.version.unwrap_or_else(|| "unknown".to_string())
        );
        println!(
            "  hook_replacement_verified: {}",
            codex.hook_replacement_verified
        );
        if !codex.hook_replacement_verified {
            println!("  mode: wrapper/observer only until output-replacement canary passes");
        } else {
            println!("  mode: hook-native eligible");
            println!("  evidence_path: {}", evidence_path.display());
        }
    }
    Ok(())
}

fn canary(args: CanaryArgs) -> Result<()> {
    if args.target != "codex-hook-replacement" {
        bail!(
            "unsupported canary `{}`; use `codex-hook-replacement`",
            args.target
        );
    }

    let paths = StorePaths::discover()?;
    paths.ensure()?;
    let result =
        cfw_codex::canary::run_output_replacement_canary(cfw_codex::canary::CanaryOptions {
            evidence_root: paths.data_dir.join("canaries"),
            codex_bin: args.codex_bin,
            model: args.model,
        })?;

    println!("Context Firewall Codex hook replacement canary");
    println!("  verified: {}", result.verified);
    println!("  reason: {}", result.reason);
    println!(
        "  codex_version: {}",
        result.codex_version.as_deref().unwrap_or("unknown")
    );
    println!("  workspace_path: {}", result.workspace_path);
    println!("  events_path: {}", result.events_path);
    println!("  last_message_path: {}", result.last_message_path);
    println!("  hook_input_path: {}", result.hook_input_path);

    if result.verified {
        let evidence_path = codex_canary_evidence_path(&paths);
        cfw_codex::canary::write_latest_verified(&evidence_path, &result)?;
        println!("  persisted_evidence_path: {}", evidence_path.display());
        return Ok(());
    }

    bail!("{}", result.reason)
}

fn current_session_id() -> String {
    std::env::var("CFW_SESSION").unwrap_or_else(|_| "local".to_string())
}

fn codex_canary_evidence_path(paths: &StorePaths) -> PathBuf {
    paths.data_dir.join("codex-hook-canary.json")
}

fn parse_line_range(range: &str) -> Result<(usize, usize)> {
    let Some((start, end)) = range.split_once(':') else {
        bail!("line range must be formatted A:B");
    };
    let start = start.parse::<usize>().context("invalid range start")?;
    let end = end.parse::<usize>().context("invalid range end")?;
    if start == 0 || end < start {
        bail!("line range must be 1-based and end must be >= start");
    }
    Ok((start, end))
}

fn guard_secret_like_output(output: &str, force: bool) -> Result<()> {
    if force || !looks_secret_like(output) {
        return Ok(());
    }
    bail!(
        "SecretGuard: output looks like it may contain credentials or private keys; rerun with `--force` only if you intentionally want raw local output"
    )
}

fn looks_secret_like(output: &str) -> bool {
    let patterns = [
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
        r"(?i)\bAWS_SECRET_ACCESS_KEY\b",
        r"\bghp_[A-Za-z0-9_]{20,}\b",
        r"\bgithub_pat_[A-Za-z0-9_]{20,}\b",
        r"\bsk-[A-Za-z0-9_-]{20,}\b",
        r"\bxox[baprs]-[A-Za-z0-9-]{20,}\b",
        r#"(?i)\b(api[_-]?key|secret|token)\s*[:=]\s*['"]?[A-Za-z0-9_./+=-]{24,}"#,
    ];
    patterns.iter().any(|pattern| {
        Regex::new(pattern)
            .expect("valid secret regex")
            .is_match(output)
    })
}

fn remove_span_artifacts(span: &SpanRecord) -> Result<usize> {
    let artifact = PathBuf::from(&span.artifact_path);
    let mut candidates = vec![artifact.clone()];
    let mut stdout_path = artifact.clone();
    stdout_path.set_extension("stdout");
    candidates.push(stdout_path);
    let mut stderr_path = artifact.clone();
    stderr_path.set_extension("stderr");
    candidates.push(stderr_path);
    let mut meta_path = artifact;
    meta_path.set_extension("meta.json");
    candidates.push(meta_path);

    let mut deleted = 0usize;
    for path in candidates {
        match std::fs::remove_file(&path) {
            Ok(()) => deleted += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("could not delete {}", path.display()));
            }
        }
    }
    Ok(deleted)
}

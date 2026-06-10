use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use cfw_core::ids::new_id;
use cfw_core::span::{DeliveryStatus, SpanRecord};
use cfw_core::token::estimate_tokens;
use cfw_policy::{Policy, PolicyAction};
use cfw_store::paths::StorePaths;
use cfw_store::sqlite::Store;
use chrono::{Duration, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use regex::Regex;

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

    /// Command and arguments to execute.
    #[arg(last = true, required = true)]
    command: Vec<String>,
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
struct ReceiptArgs {
    /// Emit JSON instead of terminal text.
    #[arg(long)]
    json: bool,
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
    }
}

fn first_run() -> Result<()> {
    eprintln!("Context Firewall first run: executing a real local command through cfw run.");
    run_command(RunArgs {
        kind: "test-output".to_string(),
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
    let decision = policy.decide_command(&args.command);
    if decision.action == PolicyAction::Block {
        bail!(
            "PolicyBlocked: {} ({})",
            decision.explanation,
            decision.reason_code
        );
    }

    let cwd = std::env::current_dir().context("CfwExecutionError: could not read cwd")?;
    let output = Command::new(program)
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("CfwExecutionError: could not run `{command_text}`"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n[stderr]\n{stderr}")
    };
    let reducer_kind = choose_reducer_kind(&args.kind, decision.reason_code);
    let reduction = cfw_reducers::reduce(reducer_kind, &raw);
    let span_kind = reducer_kind.to_string();
    let raw_estimate = estimate_tokens(&raw);
    let returned_estimate = estimate_tokens(&reduction.output);

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
    });
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)
        .with_context(|| format!("could not write {}", meta_path.display()))?;

    let hash = blake3::hash(raw.as_bytes()).to_hex().to_string();
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
        risk_class: if reduction.omitted {
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
            let decision = policy.decide_command(&command);
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
        let codex = cfw_codex::doctor::check();
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
        }
    }
    Ok(())
}

fn current_session_id() -> String {
    std::env::var("CFW_SESSION").unwrap_or_else(|_| "local".to_string())
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

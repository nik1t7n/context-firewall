use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use cfw_core::ids::new_id;
use cfw_core::span::{DeliveryStatus, SpanRecord};
use cfw_core::token::estimate_tokens;
use cfw_store::paths::StorePaths;
use cfw_store::sqlite::Store;
use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};

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
    /// Run a real command, store raw output, and print compact output.
    Run(RunArgs),
    /// Compact stdin with a deterministic reducer.
    Compact(CompactArgs),
    /// Show raw artifact output for a span.
    Show(ShowArgs),
    /// Print a local receipt from recent spans.
    Receipt(ReceiptArgs),
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

    /// Path to AGENTS.md when --write-agents is set.
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

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Install(args) => install(args),
        Commands::Run(args) => run_command(args),
        Commands::Compact(args) => compact(args),
        Commands::Show(args) => show(args),
        Commands::Receipt(args) => receipt(args),
        Commands::Top(args) => top(args),
        Commands::Doctor(args) => doctor(args),
    }
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
                let outcome = cfw_codex::install::write_wrapper_snippet(&args.agents_path)?;
                println!("  agents_path: {}", args.agents_path.display());
                println!("  result: {:?}", outcome);
            } else {
                println!();
                println!("{}", cfw_codex::install::wrapper_snippet());
            }
        }
    }
    Ok(())
}

fn run_command(args: RunArgs) -> Result<()> {
    let Some((program, rest)) = args.command.split_first() else {
        bail!("CfwExecutionError: missing command");
    };

    let cwd = std::env::current_dir().context("CfwExecutionError: could not read cwd")?;
    let output = Command::new(program)
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "CfwExecutionError: could not run `{}`",
                args.command.join(" ")
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n[stderr]\n{stderr}")
    };
    let reduction = cfw_reducers::reduce(&args.kind, &raw);
    let raw_estimate = estimate_tokens(&raw);
    let returned_estimate = estimate_tokens(&reduction.output);

    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    let session_id = current_session_id();
    let span_id = new_id();
    let session_dir = paths.sessions_dir.join(&session_id).join("artifacts");
    std::fs::create_dir_all(&session_dir)
        .with_context(|| format!("could not create {}", session_dir.display()))?;
    let artifact_path = session_dir.join(format!("{span_id}.txt"));
    std::fs::write(&artifact_path, raw.as_bytes())
        .with_context(|| format!("could not write {}", artifact_path.display()))?;

    let hash = blake3::hash(raw.as_bytes()).to_hex().to_string();
    let span = SpanRecord {
        id: span_id.clone(),
        session_id,
        kind: args.kind.clone(),
        source: "cfw_run".to_string(),
        command: Some(args.command.join(" ")),
        cwd: Some(cwd.display().to_string()),
        exit_code: output.status.code(),
        raw_bytes: raw.len() as i64,
        raw_estimated_tokens: raw_estimate.tokens,
        returned_bytes: reduction.output.len() as i64,
        returned_estimated_tokens: returned_estimate.tokens,
        hash,
        reducer: Some(reduction.reducer),
        policy_action: "compact".to_string(),
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
        for (idx, line) in artifact.lines().enumerate() {
            let line_no = idx + 1;
            if line_no >= start && line_no <= end {
                println!("{line_no}: {line}");
            }
        }
    } else {
        print!("{artifact}");
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

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::io::{BufRead, IsTerminal, Write};
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
use toml::Value as TomlValue;

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
    command: Option<Commands>,
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
    /// Search stored raw span output.
    SearchSpans(SearchSpansArgs),
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
    /// Show local token savings from recent spans.
    Gain(AnalyticsArgs),
    /// Show commands that need better Context Firewall coverage.
    Discover(AnalyticsArgs),
    /// Show recent Context Firewall session adoption and reducer mix.
    Session(AnalyticsArgs),
    /// Suggest local rules from repeated misses in the span ledger.
    Learn(AnalyticsArgs),
    /// Check local Context Firewall and Codex integration health.
    Doctor(DoctorArgs),
    /// Run real integration canaries.
    Canary(CanaryArgs),
    /// Run Context Firewall as a stdio MCP server.
    Mcp,
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

    /// Plain-text pattern to search in the stored raw artifact.
    #[arg(long)]
    grep: Option<String>,

    /// Include this many surrounding lines for --grep matches.
    #[arg(long, default_value_t = 0)]
    around: usize,

    /// Bypass secret-like output guard.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct SearchSpansArgs {
    /// Plain-text pattern to search in recent raw artifacts.
    pattern: String,

    /// Number of recent spans to inspect.
    #[arg(long, default_value_t = 50)]
    limit: i64,

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

    /// Path to the Codex AGENTS.md guidance file to inspect.
    #[arg(long, default_value = "AGENTS.md")]
    agents_path: PathBuf,
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
struct AnalyticsArgs {
    /// Number of recent spans to inspect.
    #[arg(long, default_value_t = 1000)]
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
    let Some(command) = cli.command else {
        print_launch_screen();
        return Ok(());
    };
    match command {
        Commands::Install(args) => install(args),
        Commands::Uninstall(args) => uninstall(args),
        Commands::FirstRun => first_run(),
        Commands::Run(args) => run_command(args),
        Commands::Compact(args) => compact(args),
        Commands::Show(args) => show(args),
        Commands::SearchSpans(args) => search_spans(args),
        Commands::Spans(args) => spans(args),
        Commands::Receipt(args) => receipt(args),
        Commands::Purge(args) => purge(args),
        Commands::Policy(args) => policy(args),
        Commands::Top(args) => top(args),
        Commands::Gain(args) => gain(args),
        Commands::Discover(args) => discover(args),
        Commands::Session(args) => session(args),
        Commands::Learn(args) => learn(args),
        Commands::Doctor(args) => doctor(args),
        Commands::Canary(args) => canary(args),
        Commands::Mcp => mcp(),
    }
}

fn first_run() -> Result<()> {
    print_first_run_intro();
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

fn print_launch_screen() {
    let colors = terminal_colors_enabled();
    let lines = [
        "  ____ ___  _   _ _____ _______  _______",
        " / ___/ _ \\| \\ | |_   _| ____\\ \\/ /_   _|",
        "| |  | | | |  \\| | | | |  _|  \\  /  | |",
        "| |__| |_| | |\\  | | | | |___ /  \\  | |",
        " \\____\\___/|_| \\_| |_| |_____/_/\\_\\ |_|",
        "",
        " _____ ___ ____  _______        ___    _     _",
        "|  ___|_ _|  _ \\| ____\\ \\      / / \\  | |   | |",
        "| |_   | || |_) |  _|  \\ \\ /\\ / / _ \\ | |   | |",
        "|  _|  | ||  _ <| |___  \\ V  V / ___ \\| |___| |___",
        "|_|   |___|_| \\_\\_____|  \\_/\\_/_/   \\_\\_____|_____|",
    ];
    for (index, line) in lines.iter().enumerate() {
        println!("{}", flame(line, index, colors));
    }
    println!();
    println!("{}", accent("Context Firewall", colors));
    println!(
        "{}",
        accent("Local-first token control for coding agents", colors)
    );
    println!("Stores full command output locally. Returns compact evidence to the agent.");
    println!();
    println!("{}", label("Start", colors));
    println!("  cfw first-run");
    println!("  cfw install agent");
    println!("  cfw install gemini");
    println!("  cfw install claude");
    println!("  cfw install cursor");
    println!("  cfw run -- cargo test");
    println!();
    println!("{}", label("Inspect", colors));
    println!("  cfw receipt");
    println!("  cfw top");
    println!("  cfw mcp");
}

fn print_first_run_intro() {
    let colors = terminal_colors_enabled();
    println!("{}", label("Context Firewall first run", colors));
    println!("Executing a real local command through cfw run.");
    println!();
}

fn terminal_colors_enabled() -> bool {
    match std::env::var("CFW_COLOR").as_deref() {
        Ok("always") => true,
        Ok("never") => false,
        _ => std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal(),
    }
}

fn flame(text: &str, index: usize, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }
    let colors = [
        (255, 28, 28),
        (255, 54, 24),
        (255, 83, 17),
        (255, 111, 10),
        (255, 67, 23),
    ];
    let (r, g, b) = colors[index % colors.len()];
    format!("\x1b[1;38;2;{r};{g};{b}m{text}\x1b[0m")
}

fn accent(text: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[1;38;2;255;65;24m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn label(text: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[1;38;2;255;111;10m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn install(args: InstallArgs) -> Result<()> {
    match args.target.as_str() {
        "codex" => install_codex(args),
        "agent" | "agents" => install_agent(args),
        "gemini" | "gemini-cli" => install_gemini(args),
        "antigravity" | "agy" => install_antigravity(args),
        "claude" | "claude-code" => install_claude(args),
        "cursor" | "cursor-ai" => install_cursor(args),
        _ => bail!(
            "unsupported adapter `{}`; use agent, gemini, antigravity, claude, cursor, or codex",
            args.target
        ),
    }
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

fn install_codex(args: InstallArgs) -> Result<()> {
    match args.mode {
        InstallMode::HookNative => {
            bail!(
                "HookReplacementFailed: direct output replacement is not enabled by this installer. Use `cfw install codex --mode wrapper`."
            )
        }
        InstallMode::Wrapper => {
            println!("Context Firewall agent adapter");
            println!("  target: codex");
            println!("  mcp: cfw mcp");
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
            Ok(())
        }
    }
}

fn install_agent(args: InstallArgs) -> Result<()> {
    let outcome = if args.dry_run {
        cfw_codex::install::inspect_wrapper_snippet(&args.agents_path)?
    } else {
        cfw_codex::install::write_wrapper_snippet(&args.agents_path)?
    };
    println!("Context Firewall agent adapter");
    println!("  target: agent");
    println!("  agents_path: {}", args.agents_path.display());
    println!("  dry_run: {}", args.dry_run);
    println!("  result: {:?}", outcome);
    Ok(())
}

fn install_gemini(args: InstallArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("could not read current directory")?;
    let mcp_path = cwd.join(".gemini").join("settings.json");
    let user_mcp_path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".gemini/settings.json"));
    let memory_path = cwd.join("GEMINI.md");
    let mcp_result = write_mcp_config(
        &mcp_path,
        mcp_server_config(None, false, false),
        args.dry_run,
    )?;
    let user_mcp_result = if let Some(path) = user_mcp_path.as_deref() {
        Some(write_mcp_config(
            path,
            mcp_server_config(Some(&cwd), false, false),
            args.dry_run,
        )?)
    } else {
        None
    };
    let memory_result = write_agent_block(&memory_path, args.dry_run)?;
    println!("Context Firewall agent adapter");
    println!("  target: gemini");
    println!("  mcp_path: {}", mcp_path.display());
    if let Some(path) = user_mcp_path.as_deref() {
        println!("  user_mcp_path: {}", path.display());
    }
    println!("  memory_path: {}", memory_path.display());
    println!("  dry_run: {}", args.dry_run);
    println!("  mcp_result: {:?}", mcp_result);
    if let Some(result) = user_mcp_result {
        println!("  user_mcp_result: {:?}", result);
    }
    println!("  memory_result: {:?}", memory_result);
    Ok(())
}

fn install_antigravity(args: InstallArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("could not read current directory")?;
    let mut paths = vec![cwd.join(".antigravity").join("mcp_config.json")];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.push(home.join(".gemini/antigravity-cli/mcp_config.json"));
        paths.push(home.join(".gemini/antigravity/mcp_config.json"));
    }

    println!("Context Firewall agent adapter");
    println!("  target: antigravity");
    println!("  dry_run: {}", args.dry_run);
    for path in paths {
        let config = if path.starts_with(&cwd) {
            mcp_server_config(None, false, true)
        } else {
            mcp_server_config(Some(&cwd), false, true)
        };
        let result = write_mcp_config(&path, config, args.dry_run)?;
        println!("  {}: {:?}", path.display(), result);
    }
    Ok(())
}

fn install_claude(args: InstallArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("could not read current directory")?;
    let agents_result = if args.dry_run {
        cfw_codex::install::inspect_wrapper_snippet(&args.agents_path)?
    } else {
        cfw_codex::install::write_wrapper_snippet(&args.agents_path)?
    };
    let mcp_path = cwd.join(".mcp.json");
    let claude_path = cwd.join("CLAUDE.md");
    let mcp_result = write_mcp_config(
        &mcp_path,
        mcp_server_config(None, true, false),
        args.dry_run,
    )?;
    let claude_result = write_claude_import(&claude_path, args.dry_run)?;
    println!("Context Firewall agent adapter");
    println!("  target: claude");
    println!("  agents_path: {}", args.agents_path.display());
    println!("  mcp_path: {}", mcp_path.display());
    println!("  claude_path: {}", claude_path.display());
    println!("  dry_run: {}", args.dry_run);
    println!("  agents_result: {:?}", agents_result);
    println!("  mcp_result: {:?}", mcp_result);
    println!("  claude_result: {:?}", claude_result);
    Ok(())
}

fn install_cursor(args: InstallArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("could not read current directory")?;
    let agents_result = if args.dry_run {
        cfw_codex::install::inspect_wrapper_snippet(&args.agents_path)?
    } else {
        cfw_codex::install::write_wrapper_snippet(&args.agents_path)?
    };
    let mcp_path = cwd.join(".cursor").join("mcp.json");
    let rule_path = cwd
        .join(".cursor")
        .join("rules")
        .join("context-firewall.mdc");
    let mcp_result = write_mcp_config(
        &mcp_path,
        mcp_server_config(None, false, false),
        args.dry_run,
    )?;
    let rule_result = write_cursor_rule(&rule_path, args.dry_run)?;
    println!("Context Firewall agent adapter");
    println!("  target: cursor");
    println!("  agents_path: {}", args.agents_path.display());
    println!("  mcp_path: {}", mcp_path.display());
    println!("  rule_path: {}", rule_path.display());
    println!("  dry_run: {}", args.dry_run);
    println!("  agents_result: {:?}", agents_result);
    println!("  mcp_result: {:?}", mcp_result);
    println!("  rule_result: {:?}", rule_result);
    Ok(())
}

fn mcp_server_config(
    cwd: Option<&Path>,
    include_type: bool,
    include_disabled: bool,
) -> serde_json::Value {
    let mut config = serde_json::json!({
        "command": "cfw",
        "args": ["mcp"],
    });
    if let Some(cwd) = cwd {
        config["cwd"] = serde_json::json!(cwd.display().to_string());
    }
    if include_type {
        config["type"] = serde_json::json!("stdio");
    }
    if include_disabled {
        config["disabled"] = serde_json::json!(false);
    }
    config
}

fn write_mcp_config(
    path: &Path,
    server_config: serde_json::Value,
    dry_run: bool,
) -> Result<cfw_codex::install::InstallOutcome> {
    let mut root = match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<serde_json::Value>(&content)
            .with_context(|| format!("could not parse {}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()));
        }
    };
    if !root.is_object() {
        bail!("{} must contain a JSON object", path.display());
    }
    if !root
        .get("mcpServers")
        .is_some_and(|value| value.is_object())
    {
        root["mcpServers"] = serde_json::json!({});
    }
    if root["mcpServers"]["context-firewall"] == server_config {
        return Ok(cfw_codex::install::InstallOutcome::AlreadyPresent);
    }
    root["mcpServers"]["context-firewall"] = server_config;
    if !dry_run {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&root)?)
            .with_context(|| format!("could not write {}", path.display()))?;
    }
    Ok(cfw_codex::install::InstallOutcome::Written)
}

fn write_agent_block(path: &Path, dry_run: bool) -> Result<cfw_codex::install::InstallOutcome> {
    if dry_run {
        return cfw_codex::install::inspect_wrapper_snippet(path);
    }
    cfw_codex::install::write_wrapper_snippet(path)
}

fn write_claude_import(path: &Path, dry_run: bool) -> Result<cfw_codex::install::InstallOutcome> {
    const START: &str = "<!-- context-firewall:start -->";
    const END: &str = "<!-- context-firewall:end -->";
    let snippet = format!("{START}\n@AGENTS.md\n{END}\n");
    write_managed_text(path, &snippet, START, END, dry_run)
}

fn write_cursor_rule(path: &Path, dry_run: bool) -> Result<cfw_codex::install::InstallOutcome> {
    let snippet = r#"---
alwaysApply: true
---

# Context Firewall

Use Context Firewall for noisy terminal output.

- Prefer `cfw_run` for tests, builds, git diffs, search results, file dumps,
  generated files, browser snapshots, and logs.
- Use `cfw_show` to retrieve exact span lines when needed.
- Use `cfw_receipt` to inspect recent context savings.
- Keep raw evidence on disk. Keep agent context clean.
"#;
    write_whole_file_if_changed(path, snippet, dry_run)
}

fn write_managed_text(
    path: &Path,
    snippet: &str,
    start_marker: &str,
    end_marker: &str,
    dry_run: bool,
) -> Result<cfw_codex::install::InstallOutcome> {
    let existing = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()));
        }
    };
    let next = if let (Some(start), Some(end)) =
        (existing.find(start_marker), existing.find(end_marker))
    {
        let mut next = existing.clone();
        next.replace_range(start..end + end_marker.len(), snippet.trim_end());
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next
    } else {
        let mut next = existing;
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        if !next.is_empty() {
            next.push('\n');
        }
        next.push_str(snippet);
        next
    };
    if std::fs::read_to_string(path).ok().as_deref() == Some(next.as_str()) {
        return Ok(cfw_codex::install::InstallOutcome::AlreadyPresent);
    }
    if !dry_run {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        std::fs::write(path, next)
            .with_context(|| format!("could not write {}", path.display()))?;
    }
    Ok(cfw_codex::install::InstallOutcome::Written)
}

fn write_whole_file_if_changed(
    path: &Path,
    content: &str,
    dry_run: bool,
) -> Result<cfw_codex::install::InstallOutcome> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(content) {
        return Ok(cfw_codex::install::InstallOutcome::AlreadyPresent);
    }
    if !dry_run {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        std::fs::write(path, content)
            .with_context(|| format!("could not write {}", path.display()))?;
    }
    Ok(cfw_codex::install::InstallOutcome::Written)
}

#[derive(Debug)]
struct DslReducer {
    name: String,
    match_command: Regex,
    strip_lines_matching: Vec<Regex>,
    keep_lines_matching: Vec<Regex>,
    max_lines: Option<usize>,
    tail_lines: Option<usize>,
    on_empty: Option<String>,
}

impl DslReducer {
    fn reduce(&self, input: &str) -> cfw_reducers::Reduction {
        let original_len = input.lines().count();
        let mut lines = input.lines().collect::<Vec<_>>();

        if !self.strip_lines_matching.is_empty() {
            lines.retain(|line| {
                !self
                    .strip_lines_matching
                    .iter()
                    .any(|regex| regex.is_match(line))
            });
        }
        if !self.keep_lines_matching.is_empty() {
            lines.retain(|line| {
                self.keep_lines_matching
                    .iter()
                    .any(|regex| regex.is_match(line))
            });
        }
        if let Some(tail_lines) = self.tail_lines
            && lines.len() > tail_lines
        {
            lines = lines[lines.len() - tail_lines..].to_vec();
        }
        if let Some(max_lines) = self.max_lines {
            lines.truncate(max_lines);
        }

        let mut output = if lines.is_empty() {
            self.on_empty.clone().unwrap_or_default()
        } else {
            lines.join("\n")
        };
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }

        cfw_reducers::Reduction {
            reducer: format!("dsl:{}", self.name),
            output,
            omitted: lines.len() < original_len,
            notes: vec!["applied project/user TOML line reducer".to_string()],
        }
    }
}

fn load_dsl_reducer(paths: &StorePaths, command: &str) -> Result<Option<DslReducer>> {
    for path in [
        PathBuf::from(".cfw/reducers.toml"),
        paths.data_dir.join("reducers.toml"),
    ] {
        if let Some(reducer) = load_matching_dsl_reducer(&path, command)? {
            return Ok(Some(reducer));
        }
    }
    Ok(None)
}

fn load_matching_dsl_reducer(path: &Path, command: &str) -> Result<Option<DslReducer>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()));
        }
    };
    let root = toml::from_str::<TomlValue>(&content)
        .with_context(|| format!("invalid TOML in {}", path.display()))?;
    let Some(reducers) = root.get("reducers") else {
        bail!("{} must contain [[reducers]] entries", path.display());
    };
    let reducers = reducers
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("{} `reducers` must be an array", path.display()))?;

    for (idx, reducer) in reducers.iter().enumerate() {
        let reducer = parse_dsl_reducer(path, idx, reducer)?;
        if reducer.match_command.is_match(command) {
            return Ok(Some(reducer));
        }
    }
    Ok(None)
}

fn parse_dsl_reducer(path: &Path, idx: usize, value: &TomlValue) -> Result<DslReducer> {
    const ALLOWED: &[&str] = &[
        "name",
        "match_command",
        "strip_lines_matching",
        "keep_lines_matching",
        "max_lines",
        "tail_lines",
        "on_empty",
    ];
    let table = value.as_table().ok_or_else(|| {
        anyhow::anyhow!("{} reducer #{} must be a table", path.display(), idx + 1)
    })?;
    for key in table.keys() {
        if !ALLOWED.contains(&key.as_str()) {
            bail!(
                "{} reducer #{} has unknown field `{key}`",
                path.display(),
                idx + 1
            );
        }
    }

    let name = dsl_string(table, "name")?.unwrap_or_else(|| format!("reducer-{}", idx + 1));
    let match_command = required_regex(path, &name, table, "match_command")?;
    let strip_lines_matching = regex_list(path, &name, idx, table, "strip_lines_matching")?;
    let keep_lines_matching = regex_list(path, &name, idx, table, "keep_lines_matching")?;
    Ok(DslReducer {
        name,
        match_command,
        strip_lines_matching,
        keep_lines_matching,
        max_lines: dsl_usize(path, idx, table, "max_lines")?,
        tail_lines: dsl_usize(path, idx, table, "tail_lines")?,
        on_empty: dsl_string(table, "on_empty")?,
    })
}

fn required_regex(
    path: &Path,
    name: &str,
    table: &toml::map::Map<String, TomlValue>,
    key: &str,
) -> Result<Regex> {
    let pattern = dsl_string(table, key)?
        .ok_or_else(|| anyhow::anyhow!("{} reducer `{name}` missing `{key}`", path.display()))?;
    Regex::new(&pattern).with_context(|| {
        format!(
            "{} reducer `{name}` has invalid `{key}` regex",
            path.display()
        )
    })
}

fn regex_list(
    path: &Path,
    name: &str,
    idx: usize,
    table: &toml::map::Map<String, TomlValue>,
    key: &str,
) -> Result<Vec<Regex>> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| {
        anyhow::anyhow!(
            "{} reducer #{} `{key}` must be an array",
            path.display(),
            idx + 1
        )
    })?;
    let mut regexes = Vec::new();
    for value in values {
        let pattern = value.as_str().ok_or_else(|| {
            anyhow::anyhow!(
                "{} reducer `{name}` `{key}` entries must be strings",
                path.display(),
            )
        })?;
        regexes.push(Regex::new(pattern).with_context(|| {
            format!(
                "{} reducer `{name}` has invalid `{key}` regex",
                path.display(),
            )
        })?);
    }
    Ok(regexes)
}

fn dsl_string(table: &toml::map::Map<String, TomlValue>, key: &str) -> Result<Option<String>> {
    table
        .get(key)
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow::anyhow!("`{key}` must be a string"))
        })
        .transpose()
}

fn dsl_usize(
    path: &Path,
    idx: usize,
    table: &toml::map::Map<String, TomlValue>,
    key: &str,
) -> Result<Option<usize>> {
    table
        .get(key)
        .map(|value| {
            let value = value.as_integer().ok_or_else(|| {
                anyhow::anyhow!(
                    "{} reducer #{} `{key}` must be an integer",
                    path.display(),
                    idx + 1
                )
            })?;
            usize::try_from(value).map_err(|_| {
                anyhow::anyhow!(
                    "{} reducer #{} `{key}` must be >= 0",
                    path.display(),
                    idx + 1
                )
            })
        })
        .transpose()
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
    let dsl_reducer = if args.kind == "generic" && reducer_kind == "generic" {
        load_dsl_reducer(&paths, &command_text)?
    } else {
        None
    };
    let mut reduction = if let Some(reducer) = dsl_reducer.as_ref() {
        reducer.reduce(&raw)
    } else {
        cfw_reducers::reduce(reducer_kind, &raw)
    };
    let span_kind = reduction.reducer.clone();
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
    if args.lines.is_some() && args.grep.is_some() {
        bail!("show accepts only one of --lines or --grep");
    }
    if args.around > 0 && args.grep.is_none() {
        bail!("--around requires --grep");
    }

    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    let Some(span) = store.get_span(&args.span_id)? else {
        bail!("span not found: {}", args.span_id);
    };
    let artifact = std::fs::read_to_string(&span.artifact_path)
        .with_context(|| format!("could not read {}", span.artifact_path))?;

    let selected = if let Some(range) = args.lines {
        let (start, end) = parse_line_range(&range)?;
        let mut selected = String::new();
        for (idx, line) in artifact.lines().enumerate() {
            let line_no = idx + 1;
            if line_no >= start && line_no <= end {
                selected.push_str(&format!("{line_no}: {line}\n"));
            }
        }
        selected
    } else if let Some(pattern) = args.grep {
        grep_artifact(&artifact, &pattern, args.around)
    } else {
        artifact
    };
    guard_secret_like_output(&selected, args.force)?;
    record_show_lookup(&paths, &store, &span, &selected)?;
    print!("{selected}");
    Ok(())
}

fn record_show_lookup(
    paths: &StorePaths,
    store: &Store,
    target: &SpanRecord,
    output: &str,
) -> Result<()> {
    let session_id = current_session_id();
    let cwd = std::env::current_dir().context("could not read current directory")?;
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
    std::fs::write(&artifact_path, output.as_bytes())
        .with_context(|| format!("could not write {}", artifact_path.display()))?;
    let estimate = estimate_tokens(output);
    store.insert_span(&SpanRecord {
        id: span_id,
        session_id,
        kind: "show".to_string(),
        source: "cfw_show".to_string(),
        command: Some(format!("cfw show {}", target.id)),
        cwd: Some(cwd.display().to_string()),
        exit_code: Some(0),
        raw_bytes: output.len() as i64,
        raw_estimated_tokens: estimate.tokens,
        returned_bytes: output.len() as i64,
        returned_estimated_tokens: estimate.tokens,
        hash: blake3::hash(output.as_bytes()).to_hex().to_string(),
        reducer: Some("show".to_string()),
        policy_action: "allow".to_string(),
        delivery_status: DeliveryStatus::ObservedOnly,
        delivery_evidence_path: None,
        repeat_key: String::new(),
        repeat_evidence_json: "{}".to_string(),
        risk_class: "raw_lookup".to_string(),
        artifact_path: artifact_path.display().to_string(),
        created_at: Utc::now(),
    })?;
    Ok(())
}

fn search_spans(args: SearchSpansArgs) -> Result<()> {
    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    let spans = store.recent_spans(args.limit.max(0))?;
    let mut hits = 0usize;

    for span in spans {
        let artifact = std::fs::read_to_string(&span.artifact_path)
            .with_context(|| format!("could not read {}", span.artifact_path))?;
        for (idx, line) in artifact.lines().enumerate() {
            if line.contains(&args.pattern) {
                let selected = format!("{}:{}: {line}\n", span.id, idx + 1);
                guard_secret_like_output(&selected, args.force)?;
                print!("{selected}");
                if let Some(command) = &span.command {
                    println!("   command: {}", display_command(command));
                }
                hits += 1;
            }
        }
    }

    if hits == 0 {
        println!("no matches");
    }
    Ok(())
}

fn grep_artifact(artifact: &str, pattern: &str, around: usize) -> String {
    let lines = artifact.lines().collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    for (idx, line) in lines.iter().enumerate() {
        if line.contains(pattern) {
            let start = idx.saturating_sub(around);
            let end = (idx + around + 1).min(lines.len());
            for selected_idx in start..end {
                selected.insert(selected_idx);
            }
        }
    }

    let mut output = String::new();
    let mut last = None;
    for idx in selected {
        if let Some(last_idx) = last
            && idx > last_idx + 1
        {
            output.push_str("[context-firewall: omitted unmatched lines]\n");
        }
        output.push_str(&format!("{}: {}\n", idx + 1, lines[idx]));
        last = Some(idx);
    }
    if output.is_empty() {
        output.push_str("no matches\n");
    }
    output
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

fn mcp() -> Result<()> {
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.context("could not read MCP stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                write_mcp_message(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": error.to_string()}
                }))?;
                continue;
            }
        };
        if request.get("id").is_none() {
            continue;
        }
        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let response = match handle_mcp_request(&request) {
            Ok(result) => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(error) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32603, "message": error.to_string()}
            }),
        };
        write_mcp_message(response)?;
    }
    Ok(())
}

fn write_mcp_message(value: serde_json::Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &value)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn handle_mcp_request(request: &serde_json::Value) -> Result<serde_json::Value> {
    let method = request
        .get("method")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing MCP method"))?;
    match method {
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "context-firewall", "version": env!("CARGO_PKG_VERSION")}
        })),
        "ping" => Ok(serde_json::json!({})),
        "tools/list" => Ok(serde_json::json!({"tools": mcp_tools()})),
        "tools/call" => {
            let params = request
                .get("params")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            call_mcp_tool(&params)
        }
        _ => bail!("unknown MCP method: {method}"),
    }
}

fn mcp_tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "cfw_run",
            "title": "Run command through Context Firewall",
            "description": "Run a real local command, store full stdout/stderr locally, and return compact agent-friendly output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Command argv, for example [\"cargo\", \"test\"]."
                    },
                    "kind": {"type": "string", "description": "Optional reducer kind."},
                    "cwd": {"type": "string", "description": "Optional working directory."},
                    "stdin_file": {"type": "string", "description": "Optional file to pass to stdin."}
                },
                "required": ["command"]
            }
        }),
        serde_json::json!({
            "name": "cfw_show",
            "title": "Show stored raw span output",
            "description": "Retrieve exact raw output, or exact line ranges, from a Context Firewall span.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "span_id": {"type": "string"},
                    "lines": {"type": "string", "description": "Optional 1-based range A:B."},
                    "grep": {"type": "string", "description": "Optional plain-text pattern to search within the span."},
                    "around": {"type": "integer", "minimum": 0, "description": "Optional surrounding line count for grep."},
                    "force": {"type": "boolean", "description": "Bypass secret-like output guard."},
                    "cwd": {"type": "string", "description": "Optional working directory."}
                },
                "required": ["span_id"],
                "oneOf": [
                    {
                        "not": {
                            "anyOf": [
                                {"required": ["lines"]},
                                {"required": ["grep"]},
                                {"required": ["around"]}
                            ]
                        }
                    },
                    {
                        "required": ["lines"],
                        "not": {
                            "anyOf": [
                                {"required": ["grep"]},
                                {"required": ["around"]}
                            ]
                        }
                    },
                    {
                        "required": ["grep"],
                        "not": {"required": ["lines"]}
                    }
                ]
            }
        }),
        serde_json::json!({
            "name": "cfw_search",
            "title": "Search stored Context Firewall spans",
            "description": "Search recent raw span artifacts for a plain-text pattern.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Plain-text pattern to search for."},
                    "limit": {"type": "integer", "description": "Number of recent spans to inspect."},
                    "force": {"type": "boolean", "description": "Bypass secret-like output guard."},
                    "cwd": {"type": "string", "description": "Optional working directory."}
                },
                "required": ["pattern"]
            }
        }),
        serde_json::json!({
            "name": "cfw_spans",
            "title": "List Context Firewall spans",
            "description": "List recent Context Firewall spans from the local ledger.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Maximum span count."},
                    "cwd": {"type": "string", "description": "Optional working directory."}
                }
            }
        }),
        serde_json::json!({
            "name": "cfw_gain",
            "title": "Show Context Firewall gain",
            "description": "Return recent local token savings from the span ledger.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Number of recent spans to inspect."},
                    "cwd": {"type": "string", "description": "Optional working directory."}
                }
            }
        }),
        serde_json::json!({
            "name": "cfw_discover",
            "title": "Discover Context Firewall coverage gaps",
            "description": "Return commands with low savings, repeated passthrough, large raw output, and repeated unchanged output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Number of recent spans to inspect."},
                    "cwd": {"type": "string", "description": "Optional working directory."}
                }
            }
        }),
        serde_json::json!({
            "name": "cfw_session",
            "title": "Show Context Firewall session summary",
            "description": "Return recent CFW-routed command count, reducer mix, delivery mix, and top commands.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Number of recent spans to inspect."},
                    "cwd": {"type": "string", "description": "Optional working directory."}
                }
            }
        }),
        serde_json::json!({
            "name": "cfw_receipt",
            "title": "Show Context Firewall receipt",
            "description": "Return recent token accounting and span summary as JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "cwd": {"type": "string", "description": "Optional working directory."}
                }
            }
        }),
    ]
}

fn call_mcp_tool(params: &serde_json::Value) -> Result<serde_json::Value> {
    let name = params
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing tool name"))?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let cwd = args
        .get("cwd")
        .and_then(|value| value.as_str())
        .map(PathBuf::from);

    let argv = match name {
        "cfw_run" => mcp_run_args(&args)?,
        "cfw_show" => mcp_show_args(&args)?,
        "cfw_search" => mcp_search_args(&args)?,
        "cfw_spans" => {
            let limit = mcp_limit(&args, 20);
            vec![
                "spans".to_string(),
                "--json".to_string(),
                "--limit".to_string(),
                limit.to_string(),
            ]
        }
        "cfw_gain" => mcp_limited_command("gain", &args, 1000),
        "cfw_discover" => mcp_limited_command("discover", &args, 1000),
        "cfw_session" => mcp_limited_command("session", &args, 1000),
        "cfw_receipt" => vec!["receipt".to_string(), "--json".to_string()],
        _ => bail!("unknown Context Firewall tool: {name}"),
    };
    let (success, text) = run_cfw_child(&argv, cwd.as_deref())?;
    Ok(serde_json::json!({
        "content": [{"type": "text", "text": text}],
        "isError": !success
    }))
}

fn mcp_run_args(args: &serde_json::Value) -> Result<Vec<String>> {
    let command = args
        .get("command")
        .and_then(|value| value.as_array())
        .ok_or_else(|| anyhow::anyhow!("cfw_run requires command array"))?;
    let mut argv = vec!["run".to_string()];
    if let Some(kind) = args.get("kind").and_then(|value| value.as_str()) {
        argv.extend(["--kind".to_string(), kind.to_string()]);
    }
    if let Some(stdin_file) = args.get("stdin_file").and_then(|value| value.as_str()) {
        argv.extend(["--stdin-file".to_string(), stdin_file.to_string()]);
    }
    argv.push("--".to_string());
    for item in command {
        let Some(part) = item.as_str() else {
            bail!("cfw_run command must contain only strings");
        };
        argv.push(part.to_string());
    }
    Ok(argv)
}

fn mcp_show_args(args: &serde_json::Value) -> Result<Vec<String>> {
    let span_id = args
        .get("span_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("cfw_show requires span_id"))?;
    let mut argv = vec!["show".to_string(), span_id.to_string()];
    if let Some(lines) = args.get("lines").and_then(|value| value.as_str()) {
        argv.extend(["--lines".to_string(), lines.to_string()]);
    }
    if let Some(grep) = args.get("grep").and_then(|value| value.as_str()) {
        argv.extend(["--grep".to_string(), grep.to_string()]);
    }
    if let Some(around) = args.get("around") {
        let around = around
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("cfw_show around must be >= 0"))?;
        argv.extend(["--around".to_string(), around.to_string()]);
    }
    if args.get("force").and_then(|value| value.as_bool()) == Some(true) {
        argv.push("--force".to_string());
    }
    Ok(argv)
}

fn mcp_search_args(args: &serde_json::Value) -> Result<Vec<String>> {
    let pattern = args
        .get("pattern")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("cfw_search requires pattern"))?;
    let mut argv = vec![
        "search-spans".to_string(),
        pattern.to_string(),
        "--limit".to_string(),
        mcp_limit(args, 50).to_string(),
    ];
    if args.get("force").and_then(|value| value.as_bool()) == Some(true) {
        argv.push("--force".to_string());
    }
    Ok(argv)
}

fn mcp_limited_command(command: &str, args: &serde_json::Value, default_limit: i64) -> Vec<String> {
    vec![
        command.to_string(),
        "--limit".to_string(),
        mcp_limit(args, default_limit).to_string(),
    ]
}

fn mcp_limit(args: &serde_json::Value, default_limit: i64) -> i64 {
    args.get("limit")
        .and_then(|value| value.as_i64())
        .unwrap_or(default_limit)
}

fn run_cfw_child(args: &[String], cwd: Option<&Path>) -> Result<(bool, String)> {
    let mut command = Command::new(std::env::current_exe().context("could not locate cfw binary")?);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .with_context(|| format!("could not run cfw {}", args.join(" ")))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.stderr.is_empty() {
        if !text.is_empty() {
            text.push_str("\n[stderr]\n");
        }
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    Ok((output.status.success(), text))
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

fn gain(args: AnalyticsArgs) -> Result<()> {
    let spans = recent_analytics_spans(args.limit)?;
    let totals = analytics_totals(&spans);

    println!("Context Firewall Gain");
    println!();
    println!("spans: {}", spans.len());
    println!("raw estimated tokens: {}", totals.raw);
    println!("returned estimated tokens: {}", totals.returned);
    println!("saved estimated tokens: {}", totals.saved);
    println!("reduction: {}", percent(totals.saved, totals.raw));
    Ok(())
}

fn discover(args: AnalyticsArgs) -> Result<()> {
    let spans = recent_analytics_spans(args.limit)?;
    println!("Context Firewall Discover");
    println!();

    if spans.is_empty() {
        println!("no spans yet");
        return Ok(());
    }

    let mut low_savings = spans
        .iter()
        .filter(|span| {
            span.raw_estimated_tokens >= 100
                && span.returned_estimated_tokens * 10 > span.raw_estimated_tokens * 7
        })
        .collect::<Vec<_>>();
    low_savings.sort_by_key(|span| std::cmp::Reverse(span.raw_estimated_tokens));

    println!("low savings:");
    print_span_list(&low_savings, 5);
    println!();

    let mut large_raw = spans.iter().collect::<Vec<_>>();
    large_raw.sort_by_key(|span| std::cmp::Reverse(span.raw_estimated_tokens));
    println!("largest raw outputs:");
    print_span_list(&large_raw, 5);
    println!();

    let mut repeats: BTreeMap<&str, usize> = BTreeMap::new();
    for span in &spans {
        if !span.repeat_key.is_empty() {
            *repeats.entry(&span.repeat_key).or_default() += 1;
        }
    }
    let mut repeats = repeats
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .collect::<Vec<_>>();
    repeats.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    println!("repeated unchanged output:");
    if repeats.is_empty() {
        println!("  none");
    } else {
        for (repeat_key, count) in repeats.into_iter().take(5) {
            println!("  {count}x {}", &repeat_key[..repeat_key.len().min(12)]);
        }
    }
    println!();

    let mut passthrough: BTreeMap<String, usize> = BTreeMap::new();
    for span in &spans {
        if span.risk_class == "pass_through" {
            let command = span
                .command
                .as_deref()
                .map(command_name)
                .unwrap_or_else(|| "unknown".to_string());
            *passthrough.entry(command).or_default() += 1;
        }
    }
    let mut passthrough = passthrough
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .collect::<Vec<_>>();
    passthrough.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    println!("repeated passthrough:");
    if passthrough.is_empty() {
        println!("  none");
    } else {
        for (command, count) in passthrough.into_iter().take(5) {
            println!("  {command}: {count}");
        }
    }
    Ok(())
}

fn session(args: AnalyticsArgs) -> Result<()> {
    let spans = recent_analytics_spans(args.limit)?;
    let totals = analytics_totals(&spans);
    let mut reducers: BTreeMap<&str, usize> = BTreeMap::new();
    let mut delivery: BTreeMap<&str, usize> = BTreeMap::new();
    let mut commands: BTreeMap<String, (usize, i64)> = BTreeMap::new();

    for span in &spans {
        *reducers
            .entry(span.reducer.as_deref().unwrap_or("unknown"))
            .or_default() += 1;
        *delivery.entry(span.delivery_status.as_str()).or_default() += 1;
        if let Some(command) = &span.command {
            let name = command_name(command);
            let entry = commands.entry(name).or_default();
            entry.0 += 1;
            entry.1 += span.raw_estimated_tokens;
        }
    }

    println!("Context Firewall Session");
    println!();
    println!("cfw-routed commands: {}", spans.len());
    println!("raw estimated tokens: {}", totals.raw);
    println!("returned estimated tokens: {}", totals.returned);
    println!("saved estimated tokens: {}", totals.saved);
    println!("reduction: {}", percent(totals.saved, totals.raw));
    println!();
    print_counts("reducers", reducers);
    println!();
    print_counts("delivery", delivery);
    println!();
    println!("top commands:");
    let mut commands = commands.into_iter().collect::<Vec<_>>();
    commands.sort_by_key(|(_, (_, raw))| std::cmp::Reverse(*raw));
    if commands.is_empty() {
        println!("  none");
    } else {
        for (command, (count, raw)) in commands.into_iter().take(5) {
            println!("  {command}: {count} spans, raw={raw}");
        }
    }
    Ok(())
}

fn learn(args: AnalyticsArgs) -> Result<()> {
    let spans = recent_analytics_spans(args.limit)?;
    println!("Context Firewall Learn");
    println!();

    if spans.is_empty() {
        println!("no spans yet");
        return Ok(());
    }

    println!("suggestions for AGENTS.md:");
    print_repeated_failed_commands(&spans);
    print_repeated_show_lookups(&spans);
    println!();
    println!("suggestions for .cfw/reducers.toml:");
    print_low_savings_reducers(&spans);
    print_large_generic_commands(&spans);
    println!();
    println!("mode: read-only");
    println!("apply: not implemented");
    Ok(())
}

fn print_repeated_failed_commands(spans: &[SpanRecord]) {
    let mut failures: BTreeMap<String, Vec<&SpanRecord>> = BTreeMap::new();
    for span in spans {
        if span.exit_code.unwrap_or(0) != 0
            && let Some(command) = &span.command
        {
            failures.entry(command.clone()).or_default().push(span);
        }
    }
    let mut failures = failures
        .into_iter()
        .filter(|(_, spans)| spans.len() > 1)
        .collect::<Vec<_>>();
    failures.sort_by_key(|(_, spans)| std::cmp::Reverse(spans.len()));

    println!("  repeated failed commands:");
    if failures.is_empty() {
        println!("    none");
        return;
    }
    for (command, spans) in failures.into_iter().take(5) {
        println!(
            "    - add a project note for `{}`: {} failures, spans: {}",
            display_command(&command),
            spans.len(),
            span_ids(&spans)
        );
    }
}

fn print_repeated_show_lookups(spans: &[SpanRecord]) {
    let span_by_id = spans
        .iter()
        .map(|span| (span.id.as_str(), span))
        .collect::<BTreeMap<_, _>>();
    let show_spans = spans
        .iter()
        .filter(|span| {
            span.command
                .as_deref()
                .and_then(cfw_show_target)
                .and_then(|target| span_by_id.get(target))
                .is_some_and(|target| {
                    !matches!(
                        target.reducer.as_deref().unwrap_or("unknown"),
                        "generic" | "unknown"
                    )
                })
        })
        .collect::<Vec<_>>();

    println!("  repeated raw lookups:");
    if show_spans.len() < 2 {
        println!("    none");
        return;
    }
    println!(
        "    - agent fetched raw output {} times; consider tightening reducer notes. spans: {}",
        show_spans.len(),
        span_ids(&show_spans)
    );
}

fn print_low_savings_reducers(spans: &[SpanRecord]) {
    let mut reducers: BTreeMap<String, Vec<&SpanRecord>> = BTreeMap::new();
    for span in spans {
        if span.raw_estimated_tokens >= 100
            && span.returned_estimated_tokens * 10 > span.raw_estimated_tokens * 7
        {
            reducers
                .entry(
                    span.reducer
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_default()
                .push(span);
        }
    }
    let mut reducers = reducers
        .into_iter()
        .filter(|(_, spans)| spans.len() > 1)
        .collect::<Vec<_>>();
    reducers.sort_by_key(|(_, spans)| {
        std::cmp::Reverse(
            spans
                .iter()
                .map(|span| span.raw_estimated_tokens)
                .sum::<i64>(),
        )
    });

    println!("  low-savings reducers:");
    if reducers.is_empty() {
        println!("    none");
        return;
    }
    for (reducer, spans) in reducers.into_iter().take(5) {
        println!(
            "    - tune `{reducer}` or add a TOML filter: {} low-savings spans, evidence: {}",
            spans.len(),
            span_ids(&spans)
        );
    }
}

fn print_large_generic_commands(spans: &[SpanRecord]) {
    let mut commands: BTreeMap<String, Vec<&SpanRecord>> = BTreeMap::new();
    for span in spans {
        let reducer = span.reducer.as_deref().unwrap_or("unknown");
        if span.raw_estimated_tokens >= 200
            && matches!(reducer, "generic" | "unknown")
            && let Some(command) = &span.command
        {
            commands
                .entry(command_name(command))
                .or_default()
                .push(span);
        }
    }
    let mut commands = commands.into_iter().collect::<Vec<_>>();
    commands.retain(|(_, spans)| spans.len() > 1);
    commands.sort_by_key(|(_, spans)| {
        std::cmp::Reverse(
            spans
                .iter()
                .map(|span| span.raw_estimated_tokens)
                .sum::<i64>(),
        )
    });

    println!("  repeated large generic commands:");
    if commands.is_empty() {
        println!("    none");
        return;
    }
    for (command, spans) in commands.into_iter().take(5) {
        println!(
            "    - add [[reducers]] match_command = \"{}\": {} spans, evidence: {}",
            command,
            spans.len(),
            span_ids(&spans)
        );
    }
}

fn cfw_show_target(command: &str) -> Option<&str> {
    let mut words = command.split_whitespace().map(command_name);
    if !matches!(words.next().as_deref(), Some("cfw"))
        || !matches!(words.next().as_deref(), Some("show"))
    {
        return None;
    }
    command.split_whitespace().nth(2)
}

fn span_ids(spans: &[&SpanRecord]) -> String {
    spans
        .iter()
        .take(5)
        .map(|span| span.id[..span.id.len().min(12)].to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

struct AnalyticsTotals {
    raw: i64,
    returned: i64,
    saved: i64,
}

fn recent_analytics_spans(limit: i64) -> Result<Vec<SpanRecord>> {
    let paths = StorePaths::discover()?;
    let store = Store::open(&paths)?;
    store.recent_spans(limit.max(0))
}

fn analytics_totals(spans: &[SpanRecord]) -> AnalyticsTotals {
    let raw = spans.iter().map(|span| span.raw_estimated_tokens).sum();
    let returned = spans
        .iter()
        .map(|span| span.returned_estimated_tokens)
        .sum();
    AnalyticsTotals {
        raw,
        returned,
        saved: (raw - returned).max(0),
    }
}

fn percent(part: i64, whole: i64) -> String {
    if whole <= 0 {
        "0.0%".to_string()
    } else {
        format!("{:.1}%", part as f64 * 100.0 / whole as f64)
    }
}

fn command_name(command: &str) -> String {
    command
        .split_whitespace()
        .next()
        .unwrap_or("unknown")
        .to_string()
}

fn print_counts(title: &str, counts: BTreeMap<&str, usize>) {
    println!("{title}:");
    if counts.is_empty() {
        println!("  none");
        return;
    }
    let mut counts = counts.into_iter().collect::<Vec<_>>();
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    for (name, count) in counts {
        println!("  {name}: {count}");
    }
}

fn print_span_list(spans: &[&SpanRecord], limit: usize) {
    if spans.is_empty() {
        println!("  none");
        return;
    }
    for span in spans.iter().take(limit) {
        println!(
            "  {} raw={} returned={} saved={} reducer={}",
            &span.id[..12],
            span.raw_estimated_tokens,
            span.returned_estimated_tokens,
            (span.raw_estimated_tokens - span.returned_estimated_tokens).max(0),
            span.reducer.as_deref().unwrap_or("unknown")
        );
        if let Some(command) = &span.command {
            println!("     command: {}", display_command(command));
        }
    }
}

fn display_command(command: &str) -> String {
    command.replace('\n', "\\n")
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
        "python" | "python3" | "uv" | "poetry" if command_invokes_pytest(argv) => {
            families.insert(DependencyFamily::Python);
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
        let guidance_installed = matches!(
            cfw_codex::install::inspect_wrapper_snippet(&args.agents_path)?,
            cfw_codex::install::InstallOutcome::AlreadyPresent
        );
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
        println!("  guidance_path: {}", args.agents_path.display());
        println!("  guidance_installed: {}", guidance_installed);
        println!(
            "  hook_replacement_verified: {}",
            codex.hook_replacement_verified
        );
        if !codex.hook_replacement_verified {
            println!("  auto_rewrite_status: unavailable");
            println!(
                "  auto_rewrite_reason: direct output replacement has not been verified in this environment"
            );
            println!("  mode: wrapper/observer only");
        } else {
            println!("  auto_rewrite_status: verified");
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

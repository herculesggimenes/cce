mod dashboard;

use anyhow::Result;
use cce_core::{
    AppConfig, OpenRequest, create_open_plan, ensure_config_exists, execute_open_plan,
    launch_session_restore, load_config, resolve_path,
};
use clap::{Parser, Subcommand};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Parser)]
#[command(name = "cce", bin_name = "cce", about = "Code context editor router")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Open(OpenCommand),
    Dashboard(DashboardCommand),
    InstallMacos,
    ShellInit(ShellInitCommand),
}

#[derive(Debug, Parser, Clone)]
struct OpenCommand {
    #[arg(long)]
    wait: bool,
    #[arg(long)]
    line: Option<u32>,
    #[arg(long)]
    column: Option<u32>,
    #[arg(value_name = "TARGET")]
    targets: Vec<String>,
}

#[derive(Debug, Parser, Clone)]
struct DashboardCommand {
    #[arg(long)]
    line: Option<u32>,
    #[arg(long)]
    column: Option<u32>,
    #[arg(value_name = "TARGET")]
    targets: Vec<String>,
}

#[derive(Debug, Parser)]
struct ShellInitCommand {
    shell: String,
}

#[derive(Debug, Parser)]
struct SessionCommand {
    session_query: String,
    #[arg(long)]
    cwd: Option<String>,
}

fn main() -> Result<()> {
    let argv: Vec<String> = env::args().collect();
    let args = &argv[1..];

    if args.first().map(String::as_str) == Some("resume") {
        anyhow::bail!(
            "`cce resume` was removed; use `cce <session-id-or-thread-name> [--cwd <path>]`"
        );
    }

    if args.is_empty() {
        let config = load_config()?;
        dashboard::run_dashboard(
            &config,
            OpenRequest {
                targets: vec![".".to_string()],
                line: None,
                column: None,
                wait: false,
            },
        )?;
        return Ok(());
    }

    if should_dispatch_as_session(args) {
        let session = SessionCommand::parse_from(
            std::iter::once("cce").chain(args.iter().map(String::as_str)),
        );
        let config = load_config()?;
        let cwd = session
            .cwd
            .as_deref()
            .map(|value| resolve_path(value, None));
        launch_session_restore(&config, &session.session_query, cwd.as_deref())?;
        return Ok(());
    }

    let cli = Cli::parse();
    let config = load_config()?;

    match cli.command {
        Some(Commands::Open(command)) => run_open(&config, command)?,
        Some(Commands::Dashboard(command)) => run_dashboard(&config, command)?,
        Some(Commands::InstallMacos) => run_install_macos(&config)?,
        Some(Commands::ShellInit(command)) => run_shell_init(command)?,
        None => {
            dashboard::run_dashboard(
                &config,
                OpenRequest {
                    targets: vec![".".to_string()],
                    line: None,
                    column: None,
                    wait: false,
                },
            )?;
        }
    }

    Ok(())
}

fn should_dispatch_as_session(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some(
            "-h" | "--help" | "-V" | "--version" | "open" | "dashboard" | "install-macos"
            | "shell-init",
        ) => false,
        Some(value) if value.starts_with('-') => false,
        Some(_) => true,
        None => false,
    }
}

fn run_open(config: &AppConfig, command: OpenCommand) -> Result<()> {
    let request = OpenRequest {
        targets: command.targets,
        line: command.line,
        column: command.column,
        wait: command.wait,
    };
    let plan = create_open_plan(config, &request)?;
    execute_open_plan(config, &plan)?;
    Ok(())
}

fn run_dashboard(config: &AppConfig, command: DashboardCommand) -> Result<()> {
    dashboard::run_dashboard(
        config,
        OpenRequest {
            targets: command.targets,
            line: command.line,
            column: command.column,
            wait: false,
        },
    )?;
    Ok(())
}

fn run_install_macos(config: &AppConfig) -> Result<()> {
    ensure_config_exists()?;
    let repo_root = repo_root();
    run_script(&repo_root.join("scripts/install-cce-app.sh"))?;
    run_script(&repo_root.join("scripts/set-cce-associations.sh"))?;
    if config.install.install_zed_shim {
        run_script(&repo_root.join("scripts/install-zed-shim-app.sh"))?;
    }
    Ok(())
}

fn run_script(path: &Path) -> Result<()> {
    let status = Command::new(path).status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("{} exited with {}", path.display(), status)
    }
}

fn run_shell_init(command: ShellInitCommand) -> Result<()> {
    let body = r#"export CODEX_SESSION_CWD_DIR="${CODEX_SESSION_CWD_DIR:-$HOME/.codex/session-worktrees}"
__cce_track_codex_cwd() {
  if [ -z "${CODEX_THREAD_ID:-}" ]; then
    return 0
  fi
  mkdir -p "$CODEX_SESSION_CWD_DIR" 2>/dev/null || return 0
  printf '%s\n' "$PWD" >| "$CODEX_SESSION_CWD_DIR/$CODEX_THREAD_ID.cwd"
}
__cce_track_codex_cwd"#;

    match command.shell.as_str() {
        "zsh" => {
            println!("{body}");
            println!("autoload -Uz add-zsh-hook 2>/dev/null || true");
            println!("if command -v add-zsh-hook >/dev/null 2>&1; then");
            println!("  add-zsh-hook chpwd __cce_track_codex_cwd");
            println!("  add-zsh-hook precmd __cce_track_codex_cwd");
            println!("fi");
        }
        "bash" => {
            println!("{body}");
            println!(r#"case "${{PROMPT_COMMAND:-}}" in"#);
            println!(r#"  *__cce_track_codex_cwd*) ;;"#);
            println!(r#"  "") PROMPT_COMMAND="__cce_track_codex_cwd" ;;"#);
            println!(r#"  *) PROMPT_COMMAND="__cce_track_codex_cwd; $PROMPT_COMMAND" ;;"#);
            println!("esac");
        }
        other => anyhow::bail!("unsupported shell: {other}"),
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("repo root")
}

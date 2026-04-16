use crate::model::{AppConfig, OpenAction, OpenPlan};
use crate::routing::{build_nvim_argv, nvim_bin, session_cwd, tmux_session_is_attached, which};
use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn default_shell() -> String {
    env::var("SHELL")
        .ok()
        .filter(|shell| Path::new(shell).exists())
        .or_else(|| which("zsh"))
        .or_else(|| which("bash"))
        .unwrap_or_else(|| "/bin/sh".to_string())
}

fn require_binary(name: &str) -> Result<String> {
    which(name).ok_or_else(|| anyhow::anyhow!("{name} is not available in PATH"))
}

pub fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(ch, '/' | '.' | '_' | '-' | ':' | '+' | '=' | '@' | ',')
    }) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub fn shell_join(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_checked(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to invoke `{program}`"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("`{program}` exited with {status}")
    }
}

fn run_checked_output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to invoke `{program}`"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        bail!(
            "`{program}` exited with {status}: {stderr}",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr)
        )
    }
}

fn tmux_window_target(pane_id: &str) -> Result<String> {
    run_checked_output(
        "tmux",
        &[
            "display-message",
            "-p",
            "-t",
            pane_id,
            "#{session_name}:#{window_index}",
        ],
    )
}

fn tmux_session_exists(session_name: &str) -> bool {
    if which("tmux").is_none() {
        return false;
    }
    Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn new_tmux_window(target_session: &str, workdir: &Path, window_name: &str) -> Result<String> {
    let workdir_text = workdir.display().to_string();
    if tmux_session_exists(target_session) {
        run_checked_output(
            "tmux",
            &[
                "new-window",
                "-t",
                &format!("{target_session}:"),
                "-P",
                "-F",
                "#{pane_id}",
                "-n",
                window_name,
                "-c",
                &workdir_text,
            ],
        )
    } else {
        let shell = default_shell();
        run_checked_output(
            "tmux",
            &[
                "new-session",
                "-d",
                "-s",
                target_session,
                "-P",
                "-F",
                "#{pane_id}",
                "-n",
                window_name,
                "-c",
                &workdir_text,
                &shell,
            ],
        )
    }
}

fn open_ghostty_with_command(ghostty_app: &str, shell_command: &str) {
    if env::var("CCE_DISABLE_GHOSTTY").ok().as_deref() == Some("1") {
        return;
    }
    let _ = Command::new("open")
        .args([
            "-na",
            ghostty_app,
            "--args",
            "-e",
            "/bin/sh",
            "-lc",
            shell_command,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn ghostty_application_name(ghostty_app: &str) -> String {
    let candidate = Path::new(ghostty_app)
        .file_stem()
        .or_else(|| Path::new(ghostty_app).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or(ghostty_app);
    candidate
        .strip_suffix(".app")
        .unwrap_or(candidate)
        .to_string()
}

fn activate_terminal_app(ghostty_app: &str) {
    if !cfg!(target_os = "macos") {
        return;
    }

    let app_name = ghostty_application_name(ghostty_app);
    let script = format!(
        "tell application \"{}\" to activate",
        app_name.replace('\\', "\\\\").replace('"', "\\\"")
    );

    let activated = Command::new("osascript")
        .args(["-e", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if !activated {
        let _ = Command::new("open")
            .args(["-a", ghostty_app])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

fn maybe_attach_ghostty(config: &AppConfig, session_name: &str, panes_text: &str) {
    if env::var("TMUX").is_ok() {
        return;
    }
    if tmux_session_is_attached(session_name, Some(panes_text)) {
        activate_terminal_app(&config.launcher.ghostty_app);
        return;
    }
    let command = format!("exec tmux attach-session -t {}", shell_quote(session_name));
    open_ghostty_with_command(&config.launcher.ghostty_app, &command);
}

fn temp_status_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!("cce.{timestamp}.status"))
}

pub fn launch_tmux_nvim(
    config: &AppConfig,
    session_name: &str,
    workdir: &Path,
    nvim_argv: &[String],
    wait: bool,
    panes_text: &str,
) -> Result<i32> {
    require_binary("tmux")?;
    let window_name = workdir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("nvim");
    let pane_id = new_tmux_window(session_name, workdir, window_name)?;
    let window_target = tmux_window_target(&pane_id)?;

    let mut nvim_command = shell_join(nvim_argv);
    let mut status_path = None;
    let mut channel = None;

    if wait {
        let path = temp_status_path();
        let path_text = path.display().to_string();
        let wait_channel = format!(
            "cce-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        nvim_command = format!(
            "{nvim_command}; rc=$?; printf '%s' \"$rc\" > {}; tmux wait-for -S {}; exit \"$rc\"",
            shell_quote(&path_text),
            shell_quote(&wait_channel)
        );
        status_path = Some(path);
        channel = Some(wait_channel);
    }

    run_checked("tmux", &["send-keys", "-t", &pane_id, &nvim_command, "C-m"])?;
    run_checked("tmux", &["select-window", "-t", &window_target])?;
    run_checked("tmux", &["select-pane", "-t", &pane_id])?;
    maybe_attach_ghostty(config, session_name, panes_text);

    if !wait {
        return Ok(0);
    }

    let channel = channel.as_deref().unwrap_or_default().to_string();
    run_checked("tmux", &["wait-for", &channel])?;
    let status_path = status_path.context("missing tmux wait status file")?;
    let rc = fs::read_to_string(&status_path)
        .ok()
        .and_then(|text| text.trim().parse::<i32>().ok())
        .unwrap_or(1);
    let _ = fs::remove_file(status_path);
    Ok(rc)
}

pub fn launch_plain_nvim(
    config: &AppConfig,
    workdir: &Path,
    nvim_argv: &[String],
    wait: bool,
) -> Result<i32> {
    if wait || env::var("CCE_DISABLE_GHOSTTY").ok().as_deref() == Some("1") {
        let (program, args) = nvim_argv.split_first().context("empty nvim argv")?;
        let status = Command::new(program)
            .args(args)
            .current_dir(workdir)
            .status()
            .with_context(|| format!("failed to launch `{program}`"))?;
        return Ok(status.code().unwrap_or(1));
    }

    let command = shell_join(nvim_argv);
    let shell_command = format!(
        "cd {} && exec {command}",
        shell_quote(&workdir.display().to_string())
    );
    open_ghostty_with_command(&config.launcher.ghostty_app, &shell_command);
    Ok(0)
}

pub fn execute_open_plan(config: &AppConfig, plan: &OpenPlan) -> Result<i32> {
    match &plan.action {
        OpenAction::TmuxNvim {
            session_name,
            workdir,
        } => launch_tmux_nvim(
            config,
            session_name,
            workdir,
            &plan.nvim_argv,
            plan.wait,
            &plan.panes_text,
        ),
        OpenAction::PlainNvim { workdir } => {
            launch_plain_nvim(config, workdir, &plan.nvim_argv, plan.wait)
        }
    }
}

pub fn launch_session_restore(
    config: &AppConfig,
    session_query: &str,
    cwd_override: Option<&Path>,
) -> Result<i32> {
    let session_id = crate::routing::resolve_session_id(session_query)?;
    let cwd = cwd_override
        .map(PathBuf::from)
        .unwrap_or(session_cwd(&session_id)?);
    let panes_text = crate::routing::read_tmux_panes_text();
    let target_session = crate::routing::pick_existing_tmux_session(&cwd, Some(&panes_text))
        .unwrap_or_else(|| config.launcher.tmux_default_session.clone());
    let nvim = nvim_bin(config)?;
    let nvim_argv = build_nvim_argv(&nvim, &[], None, None, true);
    launch_tmux_nvim(
        config,
        &target_session,
        &cwd,
        &nvim_argv,
        false,
        &panes_text,
    )
}

#[cfg(test)]
mod tests {
    use super::ghostty_application_name;

    #[test]
    fn ghostty_application_name_strips_bundle_suffixes_and_paths() {
        assert_eq!(ghostty_application_name("Ghostty.app"), "Ghostty");
        assert_eq!(
            ghostty_application_name("/Applications/Ghostty.app"),
            "Ghostty"
        );
        assert_eq!(ghostty_application_name("Ghostty"), "Ghostty");
    }
}

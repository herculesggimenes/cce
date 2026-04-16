use crate::model::{
    AppConfig, LiveSessionEntry, OpenAction, OpenPlan, OpenRequest, ParsedTarget, SessionSummary,
    TmuxPane,
};
use anyhow::{Context, Result, bail};
use glob::glob;
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const DEFAULT_CONFIG_TOML: &str = include_str!("../config/default-config.toml");

pub fn config_path() -> PathBuf {
    env::var_os("CCE_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config/cce/config.toml"))
}

pub fn ensure_config_exists() -> Result<PathBuf> {
    let path = config_path();
    if path.exists() {
        return Ok(path);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&path, DEFAULT_CONFIG_TOML)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path();
    if !path.is_file() {
        return Ok(AppConfig::default());
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: AppConfig =
        toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(parsed)
}

pub fn session_cwd_dir() -> PathBuf {
    env::var_os("CCE_SESSION_CWD_DIR")
        .or_else(|| env::var_os("CODEX_SESSION_CWD_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".codex/session-worktrees"))
}

pub fn session_index_path() -> PathBuf {
    env::var_os("CCE_SESSION_INDEX_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".codex/session_index.jsonl"))
}

pub fn archived_sessions_dir() -> PathBuf {
    env::var_os("CCE_ARCHIVED_SESSIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".codex/archived_sessions"))
}

pub fn sessions_dir() -> PathBuf {
    env::var_os("CCE_SESSIONS_DIR")
        .or_else(|| env::var_os("CODEX_SESSIONS_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".codex/sessions"))
}

pub fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };

    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

pub fn resolve_path(value: &str, cwd: Option<&Path>) -> PathBuf {
    let mut path = PathBuf::from(value);
    if let Some(stripped) = value.strip_prefix("~/") {
        path = home_dir().join(stripped);
    }
    if !path.is_absolute() {
        path = cwd
            .map(PathBuf::from)
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join(path);
    }
    normalize_path(&path)
}

pub fn path_contains(parent: &Path, child: &Path) -> bool {
    let parent = normalize_path(parent);
    let child = normalize_path(child);
    child.starts_with(parent)
}

pub fn parse_target_spec(value: &str, cwd: Option<&Path>) -> ParsedTarget {
    let mut path_value = value.to_string();
    let mut line = None;
    let mut column = None;

    let parts: Vec<&str> = value.rsplitn(3, ':').collect();
    if parts.len() == 3
        && parts[0].chars().all(|ch| ch.is_ascii_digit())
        && parts[1].chars().all(|ch| ch.is_ascii_digit())
    {
        let candidate_path = parts[2];
        let candidate = resolve_path(candidate_path, cwd);
        if candidate.exists() {
            path_value = candidate_path.to_string();
            line = parts[1].parse::<u32>().ok();
            column = parts[0].parse::<u32>().ok();
        }
    } else if parts.len() == 2 && parts[0].chars().all(|ch| ch.is_ascii_digit()) {
        let candidate_path = parts[1];
        let candidate = resolve_path(candidate_path, cwd);
        if candidate.exists() {
            path_value = candidate_path.to_string();
            line = parts[0].parse::<u32>().ok();
        }
    }

    ParsedTarget {
        path: resolve_path(&path_value, cwd),
        line,
        column,
    }
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut entries = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            entries.push(value);
        }
    }
    Ok(entries)
}

pub fn load_live_session_entries(
    session_dir: Option<&Path>,
    index_path: Option<&Path>,
) -> Result<Vec<LiveSessionEntry>> {
    let default_session_dir = session_cwd_dir();
    let default_index_path = session_index_path();
    let session_dir = session_dir.unwrap_or(default_session_dir.as_path());
    let index_path = index_path.unwrap_or(default_index_path.as_path());

    let mut index_entries: BTreeMap<String, (String, String)> = BTreeMap::new();
    for entry in read_jsonl(index_path)? {
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            let thread_name = entry
                .get("thread_name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let updated_at = entry
                .get("updated_at")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            index_entries.insert(id.to_string(), (thread_name, updated_at));
        }
    }

    let mut sessions = Vec::new();
    if !session_dir.exists() {
        return Ok(sessions);
    }

    for entry in fs::read_dir(session_dir)
        .with_context(|| format!("failed to read {}", session_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("cwd") {
            continue;
        }
        let session_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_string();
        let cwd_text = fs::read_to_string(&path).unwrap_or_default();
        let cwd_line = cwd_text.lines().next().unwrap_or_default();
        if cwd_line.is_empty() {
            continue;
        }
        let cwd = resolve_path(cwd_line, None);
        let (thread_name, updated_at) = index_entries.get(&session_id).cloned().unwrap_or_default();
        sessions.push(LiveSessionEntry {
            id: session_id,
            cwd,
            thread_name,
            updated_at,
        });
    }

    sessions.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(sessions)
}

pub fn pick_live_session_for_target(
    target: &Path,
    entries: &[LiveSessionEntry],
) -> Option<LiveSessionEntry> {
    let target = normalize_path(target);
    let mut matches: Vec<LiveSessionEntry> = entries
        .iter()
        .filter(|entry| path_contains(&entry.cwd, &target))
        .cloned()
        .collect();
    matches.sort_by(|left, right| {
        let left_depth = left.cwd.components().count();
        let right_depth = right.cwd.components().count();
        right_depth
            .cmp(&left_depth)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });
    matches.into_iter().next()
}

pub fn read_tmux_panes_text() -> String {
    if let Some(override_text) = env::var_os("CCE_TMUX_LIST_PANES") {
        return override_text.to_string_lossy().to_string();
    }
    if which("tmux").is_none() {
        return String::new();
    }
    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{session_name}\t#{window_name}\t#{pane_id}\t#{pane_current_path}\t#{pane_current_command}\t#{window_active}\t#{session_attached}\t#{session_last_attached}\t#{session_activity}",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        _ => String::new(),
    }
}

pub fn parse_tmux_panes(panes_text: &str) -> Vec<TmuxPane> {
    panes_text
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 9 {
                return None;
            }
            Some(TmuxPane {
                session_name: parts[0].to_string(),
                window_name: parts[1].to_string(),
                pane_id: parts[2].to_string(),
                pane_path: resolve_path(parts[3], None),
                pane_command: parts[4].to_string(),
                window_active: parts[5].parse().unwrap_or_default(),
                attached: parts[6].parse().unwrap_or_default(),
                last_attached: parts[7].parse().unwrap_or_default(),
                activity: parts[8].parse().unwrap_or_default(),
            })
        })
        .collect()
}

pub fn tmux_session_summaries(panes_text: Option<&str>) -> Vec<SessionSummary> {
    let source = panes_text
        .map(ToString::to_string)
        .unwrap_or_else(read_tmux_panes_text);
    let mut sessions: BTreeMap<String, SessionSummary> = BTreeMap::new();
    for pane in parse_tmux_panes(&source) {
        let entry = sessions
            .entry(pane.session_name.clone())
            .or_insert(SessionSummary {
                name: pane.session_name.clone(),
                attached: 0,
                last_attached: 0,
                activity: 0,
            });
        entry.attached = entry.attached.max(pane.attached);
        entry.last_attached = entry.last_attached.max(pane.last_attached);
        entry.activity = entry.activity.max(pane.activity);
    }
    sessions.into_values().collect()
}

pub fn pick_tmux_session_for_worktree(worktree: &Path, panes_text: &str) -> Option<String> {
    let mut best_score = i32::MIN;
    let mut best_session = None;
    for pane in parse_tmux_panes(panes_text) {
        let mut score = 0;
        if path_contains(worktree, &pane.pane_path) {
            score += 250;
        } else if path_contains(&pane.pane_path, worktree) {
            score += 100;
        }
        if pane.window_name.starts_with("cce:") || pane.window_name.starts_with("cx:") {
            score += 80;
        } else if pane.window_name == "nvim" {
            score += 20;
        }
        score += pane.attached * 10;
        score += pane.last_attached.min(100);
        score += pane.activity.min(100);
        if score > best_score {
            best_score = score;
            best_session = Some(pane.session_name);
        }
    }
    best_session
}

pub fn pick_existing_tmux_session(worktree: &Path, panes_text: Option<&str>) -> Option<String> {
    let source = panes_text
        .map(ToString::to_string)
        .unwrap_or_else(read_tmux_panes_text);
    if let Some(worktree_match) = pick_tmux_session_for_worktree(worktree, &source) {
        return Some(worktree_match);
    }

    let mut sessions = tmux_session_summaries(Some(&source));
    if sessions.is_empty() {
        return None;
    }

    sessions.sort_by(|left, right| {
        right
            .attached
            .cmp(&left.attached)
            .then_with(|| right.last_attached.cmp(&left.last_attached))
            .then_with(|| right.activity.cmp(&left.activity))
            .then_with(|| right.name.cmp(&left.name))
    });
    sessions.first().map(|session| session.name.clone())
}

pub fn tmux_session_is_attached(session_name: &str, panes_text: Option<&str>) -> bool {
    tmux_session_summaries(panes_text)
        .iter()
        .any(|session| session.name == session_name && session.attached > 0)
}

pub fn resolve_session_id(query: &str) -> Result<String> {
    let query = query.trim();
    if query.is_empty() {
        bail!("usage: cce <session-id-or-thread-name> [--cwd <path>]");
    }

    let index_entries = read_jsonl(&session_index_path())?;
    if index_entries.is_empty() {
        return Ok(query.to_string());
    }

    if let Some(exact_id) = index_entries.iter().find_map(|entry| {
        entry
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| *id == query)
    }) {
        return Ok(exact_id.to_string());
    }

    if let Some(exact_name) = index_entries.iter().find_map(|entry| {
        let id = entry.get("id").and_then(Value::as_str)?;
        let thread_name = entry.get("thread_name").and_then(Value::as_str)?;
        (thread_name == query).then(|| id.to_string())
    }) {
        return Ok(exact_name);
    }

    let mut prefix_matches: Vec<String> = index_entries
        .iter()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .filter(|id| id.starts_with(query))
        .map(ToString::to_string)
        .collect();
    prefix_matches.sort();
    prefix_matches.dedup();

    match prefix_matches.len() {
        0 => bail!("Codex session not found: {query}"),
        1 => Ok(prefix_matches.remove(0)),
        _ => bail!("ambiguous Codex session id prefix"),
    }
}

pub fn session_cwd(session_id: &str) -> Result<PathBuf> {
    let live_path = session_cwd_dir().join(format!("{session_id}.cwd"));
    if live_path.is_file() {
        let text = fs::read_to_string(&live_path)
            .with_context(|| format!("failed to read {}", live_path.display()))?;
        if let Some(first_line) = text.lines().next() {
            if !first_line.trim().is_empty() {
                return Ok(resolve_path(first_line, None));
            }
        }
    }

    if env::var("CODEX_THREAD_ID").ok().as_deref() == Some(session_id) {
        return Ok(env::current_dir().context("failed to read current directory")?);
    }

    for (root, pattern) in [
        (sessions_dir(), format!("*/*/*/*{session_id}.jsonl")),
        (archived_sessions_dir(), format!("*{session_id}.jsonl")),
    ] {
        if !root.exists() {
            continue;
        }
        let glob_pattern = root.join(pattern).display().to_string();
        let mut candidates: Vec<PathBuf> = glob(&glob_pattern)
            .with_context(|| format!("invalid glob pattern {glob_pattern}"))?
            .filter_map(|entry| entry.ok())
            .collect();
        candidates.sort();
        candidates.reverse();

        for path in candidates {
            let mut fallback_cwd = None;
            for entry in read_jsonl(&path)? {
                let entry_type = entry
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let payload = entry.get("payload").cloned().unwrap_or(Value::Null);
                if entry_type == "session_meta"
                    && payload.get("id").and_then(Value::as_str) == Some(session_id)
                    && payload.get("cwd").and_then(Value::as_str).is_some()
                {
                    return Ok(resolve_path(
                        payload
                            .get("cwd")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                        None,
                    ));
                }
                if entry_type == "turn_context" && fallback_cwd.is_none() {
                    fallback_cwd = payload
                        .get("cwd")
                        .and_then(Value::as_str)
                        .map(|cwd| resolve_path(cwd, None));
                }
            }
            if let Some(fallback_cwd) = fallback_cwd {
                return Ok(fallback_cwd);
            }
        }
    }

    bail!(
        "Could not resolve a worktree path for this Codex session. Open the session once in its worktree so it gets tracked, or pass an explicit directory."
    )
}

pub fn nvim_bin(config: &AppConfig) -> Result<String> {
    if !config.launcher.nvim_bin.is_empty() {
        return Ok(config.launcher.nvim_bin.clone());
    }
    which("nvim").ok_or_else(|| anyhow::anyhow!("nvim is not available in PATH"))
}

pub fn build_nvim_argv(
    nvim: &str,
    targets: &[String],
    line: Option<u32>,
    column: Option<u32>,
    restore: bool,
) -> Vec<String> {
    let mut argv = vec![nvim.to_string()];
    if restore {
        argv.push("+lua local ok, persistence = pcall(require, 'persistence'); if ok then persistence.load() end".to_string());
    } else if let (Some(line), Some(column)) = (line, column) {
        argv.push(format!("+call cursor({line},{column})"));
    } else if let Some(line) = line {
        argv.push(format!("+{line}"));
    }
    if targets.is_empty() {
        argv.push(".".to_string());
    } else {
        argv.extend(targets.iter().cloned());
    }
    argv
}

pub fn create_open_plan(config: &AppConfig, request: &OpenRequest) -> Result<OpenPlan> {
    let live_sessions = load_live_session_entries(None, None)?;
    let panes_text = read_tmux_panes_text();
    create_open_plan_with_sources(config, request, &live_sessions, &panes_text)
}

pub fn create_open_plan_with_sources(
    config: &AppConfig,
    request: &OpenRequest,
    live_sessions: &[LiveSessionEntry],
    panes_text: &str,
) -> Result<OpenPlan> {
    let targets = if request.targets.is_empty() {
        vec![".".to_string()]
    } else {
        request.targets.clone()
    };
    let parsed_targets: Vec<ParsedTarget> = targets
        .iter()
        .map(|value| parse_target_spec(value, None))
        .collect();
    let line = request
        .line
        .or(parsed_targets.first().and_then(|target| target.line));
    let column = request
        .column
        .or(parsed_targets.first().and_then(|target| target.column));
    let target_paths: Vec<String> = parsed_targets
        .iter()
        .map(|target| target.path.display().to_string())
        .collect();
    let routing_target = parsed_targets
        .first()
        .map(|target| target.path.clone())
        .unwrap_or_else(|| resolve_path(".", None));
    let workdir = if routing_target.is_dir() {
        routing_target.clone()
    } else {
        routing_target
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| resolve_path(".", None))
    };

    let live_match = pick_live_session_for_target(&routing_target, live_sessions);
    let tmux_sessions = tmux_session_summaries(Some(panes_text));
    let nvim = nvim_bin(config)?;
    let nvim_argv = build_nvim_argv(&nvim, &target_paths, line, column, false);

    let action = if let Some(live_match) = &live_match {
        let session_name = pick_existing_tmux_session(&live_match.cwd, Some(panes_text))
            .unwrap_or_else(|| config.launcher.tmux_default_session.clone());
        OpenAction::TmuxNvim {
            session_name,
            workdir: live_match.cwd.clone(),
        }
    } else if let Some(session_name) = pick_existing_tmux_session(&workdir, Some(panes_text)) {
        OpenAction::TmuxNvim {
            session_name,
            workdir: workdir.clone(),
        }
    } else {
        OpenAction::PlainNvim {
            workdir: workdir.clone(),
        }
    };

    Ok(OpenPlan {
        parsed_targets,
        target_paths,
        routing_target,
        workdir,
        line,
        column,
        wait: request.wait,
        live_match,
        tmux_sessions,
        panes_text: panes_text.to_string(),
        nvim_argv,
        action,
    })
}

pub fn which(binary: &str) -> Option<String> {
    if binary.contains('/') {
        let path = PathBuf::from(binary);
        return path.is_file().then(|| path.display().to_string());
    }

    let path_var = env::var_os("PATH")?;
    for segment in env::split_paths(&path_var) {
        let candidate = segment.join(binary);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

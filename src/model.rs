use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub launcher: LauncherConfig,
    pub install: InstallConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            launcher: LauncherConfig::default(),
            install: InstallConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LauncherConfig {
    pub ghostty_app: String,
    pub nvim_bin: String,
    pub tmux_default_session: String,
    pub tmux_window_prefix: String,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            ghostty_app: "Ghostty.app".to_string(),
            nvim_bin: String::new(),
            tmux_default_session: "main".to_string(),
            tmux_window_prefix: "cce".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct InstallConfig {
    pub install_zed_shim: bool,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            install_zed_shim: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTarget {
    pub path: PathBuf,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveSessionEntry {
    pub id: String,
    pub cwd: PathBuf,
    pub thread_name: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxPane {
    pub session_name: String,
    pub window_name: String,
    pub pane_id: String,
    pub pane_path: PathBuf,
    pub pane_command: String,
    pub window_active: i32,
    pub attached: i32,
    pub last_attached: i32,
    pub activity: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub name: String,
    pub attached: i32,
    pub last_attached: i32,
    pub activity: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAction {
    TmuxNvim {
        session_name: String,
        workdir: PathBuf,
    },
    PlainNvim {
        workdir: PathBuf,
    },
}

impl OpenAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TmuxNvim { .. } => "tmux + nvim",
            Self::PlainNvim { .. } => "plain nvim",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenRequest {
    pub targets: Vec<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub wait: bool,
}

#[derive(Debug, Clone)]
pub struct OpenPlan {
    pub parsed_targets: Vec<ParsedTarget>,
    pub target_paths: Vec<String>,
    pub routing_target: PathBuf,
    pub workdir: PathBuf,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub wait: bool,
    pub live_match: Option<LiveSessionEntry>,
    pub tmux_sessions: Vec<SessionSummary>,
    pub panes_text: String,
    pub nvim_argv: Vec<String>,
    pub action: OpenAction,
}

pub mod launch;
pub mod model;
pub mod routing;

pub use launch::{execute_open_plan, launch_plain_nvim, launch_session_restore, launch_tmux_nvim};
pub use model::{
    AppConfig, LiveSessionEntry, OpenAction, OpenPlan, OpenRequest, ParsedTarget, SessionSummary,
};
pub use routing::{
    build_nvim_argv, config_path, create_open_plan, create_open_plan_with_sources,
    ensure_config_exists, load_config, load_live_session_entries, parse_target_spec,
    pick_existing_tmux_session, pick_live_session_for_target, pick_tmux_session_for_worktree,
    read_tmux_panes_text, resolve_path, resolve_session_id, session_cwd, tmux_session_is_attached,
    tmux_session_summaries,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn pick_live_session_prefers_deepest_match() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("repo");
        let nested = root.join("nested");
        let target = nested.join("file.py");
        fs::create_dir_all(&nested).unwrap();
        fs::write(&target, "x").unwrap();

        let entries = vec![
            LiveSessionEntry {
                id: "one".to_string(),
                cwd: root.clone(),
                thread_name: "root".to_string(),
                updated_at: "2026-03-23T10:00:00Z".to_string(),
            },
            LiveSessionEntry {
                id: "two".to_string(),
                cwd: nested.clone(),
                thread_name: "nested".to_string(),
                updated_at: "2026-03-23T09:00:00Z".to_string(),
            },
        ];

        let match_entry = pick_live_session_for_target(&target, &entries).unwrap();
        assert_eq!(match_entry.id, "two");
    }

    #[test]
    fn pick_existing_tmux_session_prefers_worktree_related_pane() {
        let worktree = PathBuf::from("/Users/hgimenes/src/project");
        let panes_text = [
            "main\tnvim\t%1\t/Users/hgimenes\tzsh\t0\t0\t0\t0",
            "work\tcce:proj\t%2\t/Users/hgimenes/src/project\ttmux\t0\t1\t20\t20",
        ]
        .join("\n");

        let target = pick_existing_tmux_session(&worktree, Some(&panes_text)).unwrap();
        assert_eq!(target, "work");
    }

    #[test]
    fn tmux_session_is_attached_detects_attached_session() {
        let panes_text = [
            "main\tnvim\t%1\t/Users/hgimenes\tzsh\t0\t0\t5\t5",
            "dev\tzsh\t%2\t/Users/hgimenes/src/other\tzsh\t1\t1\t20\t20",
        ]
        .join("\n");

        assert!(tmux_session_is_attached("dev", Some(&panes_text)));
        assert!(!tmux_session_is_attached("main", Some(&panes_text)));
    }

    #[test]
    fn parse_target_handles_line_and_column_suffixes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("demo.txt");
        fs::write(&path, "x").unwrap();

        let parsed = parse_target_spec(&format!("{}:12:4", path.display()), None);
        assert_eq!(parsed.path, resolve_path(&path.display().to_string(), None));
        assert_eq!(parsed.line, Some(12));
        assert_eq!(parsed.column, Some(4));
    }

    #[test]
    fn build_nvim_command_uses_cursor_when_line_and_column_are_present() {
        let cmd = build_nvim_argv(
            "nvim",
            &["/tmp/demo.py".to_string()],
            Some(8),
            Some(3),
            false,
        );
        assert_eq!(cmd[0], "nvim");
        assert_eq!(cmd[1], "+call cursor(8,3)");
        assert_eq!(cmd[2], "/tmp/demo.py");
    }

    #[test]
    fn open_plan_live_match_routes_into_tmux_without_codex_split() {
        let mut config = AppConfig::default();
        config.launcher.nvim_bin = "nvim".to_string();
        let live_cwd = PathBuf::from("/tmp/live-worktree");
        let live_sessions = vec![LiveSessionEntry {
            id: "abc123".to_string(),
            cwd: live_cwd.clone(),
            thread_name: "demo".to_string(),
            updated_at: "2026-03-23T10:00:00Z".to_string(),
        }];
        let request = OpenRequest {
            targets: vec!["/tmp/live-worktree/file.py".to_string()],
            line: None,
            column: None,
            wait: false,
        };

        let plan = create_open_plan_with_sources(&config, &request, &live_sessions, "").unwrap();
        match plan.action {
            OpenAction::TmuxNvim {
                session_name,
                workdir,
            } => {
                assert_eq!(session_name, "main");
                assert_eq!(workdir, live_cwd);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }
}

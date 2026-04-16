use anyhow::Result;
use cce_core::{AppConfig, OpenPlan, OpenRequest, create_open_plan, execute_open_plan};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use std::io::{self, Stdout};
use std::time::Duration;

struct DashboardState {
    request: OpenRequest,
    plan: OpenPlan,
    status: String,
}

impl DashboardState {
    fn new(config: &AppConfig, request: OpenRequest) -> Result<Self> {
        let plan = create_open_plan(config, &request)?;
        Ok(Self {
            request,
            plan,
            status: "Press Enter to route, r to refresh, q to quit.".to_string(),
        })
    }

    fn refresh(&mut self, config: &AppConfig) -> Result<()> {
        self.plan = create_open_plan(config, &self.request)?;
        self.status = "Route refreshed.".to_string();
        Ok(())
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub fn run_dashboard(config: &AppConfig, request: OpenRequest) -> Result<i32> {
    let mut state = DashboardState::new(config, request)?;
    let mut terminal = TerminalGuard::enter()?;

    loop {
        terminal
            .terminal_mut()
            .draw(|frame| render(frame.area(), frame, &state))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(0),
                KeyCode::Char('r') => {
                    state.refresh(config)?;
                }
                KeyCode::Enter | KeyCode::Char('o') => {
                    drop(terminal);
                    return execute_open_plan(config, &state.plan);
                }
                _ => {}
            }
        }
    }
}

fn render(area: Rect, frame: &mut ratatui::Frame<'_>, state: &DashboardState) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "CCE Router",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("Rust rewrite for routing files into tmux + Neovim."),
        Line::from(format!("Target: {}", state.plan.routing_target.display())),
    ])
    .block(Block::default().borders(Borders::ALL).title("Overview"));
    frame.render_widget(header, layout[0]);

    let live_match = state
        .plan
        .live_match
        .as_ref()
        .map(|session| format!("{} ({})", session.id, session.cwd.display()))
        .unwrap_or_else(|| "none".to_string());
    let tmux_target = match &state.plan.action {
        cce_core::OpenAction::TmuxNvim {
            session_name,
            workdir,
        } => format!("{session_name} -> {}", workdir.display()),
        cce_core::OpenAction::PlainNvim { workdir } => format!("plain -> {}", workdir.display()),
    };
    let route_summary = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Action: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(state.plan.action.label()),
        ]),
        Line::from(vec![
            Span::styled(
                "Live match: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(live_match),
        ]),
        Line::from(vec![
            Span::styled(
                "Target session: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(tmux_target),
        ]),
        Line::from(vec![
            Span::styled("Command: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(state.plan.nvim_argv.join(" ")),
        ]),
    ])
    .wrap(Wrap { trim: true })
    .block(Block::default().borders(Borders::ALL).title("Plan"));
    frame.render_widget(route_summary, layout[1]);

    let lists = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[2]);

    let live_items = if state.plan.live_match.is_some() {
        state
            .plan
            .live_match
            .iter()
            .map(|entry| ListItem::new(format!("{}  {}", entry.id, entry.cwd.display())))
            .collect::<Vec<_>>()
    } else {
        vec![ListItem::new("No matching live Codex worktree")]
    };
    let live_list = List::new(live_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Matched Worktree"),
    );
    frame.render_widget(live_list, lists[0]);

    let tmux_items = if state.plan.tmux_sessions.is_empty() {
        vec![ListItem::new("No tmux sessions discovered")]
    } else {
        state
            .plan
            .tmux_sessions
            .iter()
            .map(|session| {
                ListItem::new(format!(
                    "{}  attached={} activity={} last={}",
                    session.name, session.attached, session.activity, session.last_attached
                ))
            })
            .collect::<Vec<_>>()
    };
    let tmux_list = List::new(tmux_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("tmux Sessions"),
    );
    frame.render_widget(tmux_list, lists[1]);

    frame.render_widget(Clear, layout[3]);
    let footer = Paragraph::new(state.status.as_str())
        .block(Block::default().borders(Borders::ALL).title("Controls"))
        .style(Style::default().fg(Color::White));
    frame.render_widget(footer, layout[3]);
}

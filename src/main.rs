use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
};
use serde::Deserialize;
use std::{
    env, io,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(name = "repo-syncer")]
#[command(about = "Interactive TUI to sync GitHub forks with their upstream repositories")]
struct Args {
    /// Home directory for cloned repos (default: $HOME/dev)
    #[arg(long, env = "TOOL_HOME")]
    tool_home: Option<PathBuf>,

    /// Dry run - show what would be done without making changes
    #[arg(long)]
    dry_run: bool,

    /// Skip confirmation modal
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GhFork {
    name: String,
    owner: GhOwner,
    parent: Option<GhParent>,
    default_branch_ref: Option<GhBranchRef>,
}

#[derive(Debug, Deserialize, Clone)]
struct GhOwner {
    login: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GhParent {
    name: String,
    owner: GhOwner,
}

#[derive(Debug, Deserialize, Clone)]
struct GhBranchRef {
    name: String,
}

#[derive(Debug, Clone)]
struct Fork {
    name: String,
    owner: String,
    parent_owner: String,
    parent_name: String,
    default_branch: String,
    local_path: PathBuf,
    is_cloned: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum SyncStatus {
    Pending,
    Checking,
    Cloning,
    Stashing,
    Fetching,
    Syncing,
    Restoring,
    Synced,
    Skipped(String),
    Failed(String),
}

impl SyncStatus {
    fn display(&self) -> &str {
        match self {
            Self::Pending => "Pending",
            Self::Checking => "Checking",
            Self::Cloning => "Cloning",
            Self::Stashing => "Stashing",
            Self::Fetching => "Fetching",
            Self::Syncing => "Syncing",
            Self::Restoring => "Restoring",
            Self::Synced => "Synced",
            Self::Skipped(reason) | Self::Failed(reason) => reason,
        }
    }
}

struct App {
    forks: Vec<Fork>,
    statuses: Vec<SyncStatus>,
    state: TableState,
    selected: Vec<bool>,
    mode: Mode,
    dry_run: bool,
    tool_home: PathBuf,
    spinner_tick: usize,
    last_tick: Instant,
    modal_button: usize,
}

#[derive(PartialEq)]
enum Mode {
    Selecting,
    ConfirmModal,
    Syncing,
    Done,
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl App {
    fn new(forks: Vec<Fork>, dry_run: bool, tool_home: PathBuf) -> Self {
        let len = forks.len();
        let mut state = TableState::default();
        if !forks.is_empty() {
            state.select(Some(0));
        }
        Self {
            forks,
            statuses: vec![SyncStatus::Pending; len],
            state,
            selected: vec![false; len],
            mode: Mode::Selecting,
            dry_run,
            tool_home,
            spinner_tick: 0,
            last_tick: Instant::now(),
            modal_button: 1,
        }
    }

    fn next(&mut self) {
        if self.forks.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => (i + 1) % self.forks.len(),
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.forks.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.forks.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn toggle_selection(&mut self) {
        if let Some(i) = self.state.selected() {
            self.selected[i] = !self.selected[i];
        }
    }

    fn select_all(&mut self) {
        let all_selected = self.selected.iter().all(|&s| s);
        for s in &mut self.selected {
            *s = !all_selected;
        }
    }

    fn selected_count(&self) -> usize {
        self.selected.iter().filter(|&&s| s).count()
    }

    fn tick_spinner(&mut self) {
        if self.last_tick.elapsed() >= Duration::from_millis(80) {
            self.spinner_tick = (self.spinner_tick + 1) % SPINNER_FRAMES.len();
            self.last_tick = Instant::now();
        }
    }

    fn spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_tick]
    }

    fn mark_selected_as_pending(&mut self) {
        for (i, selected) in self.selected.iter().enumerate() {
            if *selected {
                self.statuses[i] = SyncStatus::Pending;
            }
        }
    }

    fn is_all_done(&self) -> bool {
        self.statuses.iter().enumerate().all(|(i, status)| {
            !self.selected[i]
                || matches!(
                    status,
                    SyncStatus::Synced | SyncStatus::Skipped(_) | SyncStatus::Failed(_)
                )
        })
    }

    fn reset_for_next_round(&mut self) {
        for i in 0..self.forks.len() {
            if matches!(self.statuses[i], SyncStatus::Synced) {
                self.selected[i] = false;
            }
            self.statuses[i] = SyncStatus::Pending;
        }
        self.modal_button = 1;
    }

    fn summary(&self) -> (usize, usize, usize) {
        let mut synced = 0;
        let mut skipped = 0;
        let mut failed = 0;
        for (i, status) in self.statuses.iter().enumerate() {
            if !self.selected[i] {
                continue;
            }
            match status {
                SyncStatus::Synced => synced += 1,
                SyncStatus::Skipped(_) => skipped += 1,
                SyncStatus::Failed(_) => failed += 1,
                _ => {}
            }
        }
        (synced, skipped, failed)
    }
}

#[derive(Debug)]
enum SyncResult {
    StatusUpdate(usize, SyncStatus),
}

fn get_tool_home(args_tool_home: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = args_tool_home {
        return Ok(path);
    }

    let home = env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join("dev"))
}

fn fetch_forks(tool_home: &Path) -> Result<Vec<Fork>> {
    let output = Command::new("gh")
        .args([
            "repo",
            "list",
            "--fork",
            "--limit",
            "200",
            "--json",
            "name,owner,parent,defaultBranchRef",
        ])
        .output()
        .context("Failed to run gh CLI. Is it installed and authenticated?")?;

    if !output.status.success() {
        anyhow::bail!(
            "gh command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let gh_forks: Vec<GhFork> = serde_json::from_slice(&output.stdout)?;

    let forks: Vec<Fork> = gh_forks
        .into_iter()
        .filter_map(|f| {
            let parent = f.parent?;
            let default_branch = f
                .default_branch_ref
                .map_or_else(|| "main".to_string(), |b| b.name);

            let local_path = tool_home.join(&f.owner.login).join(&f.name);
            let is_cloned = local_path.exists();

            Some(Fork {
                name: f.name,
                owner: f.owner.login,
                parent_owner: parent.owner.login,
                parent_name: parent.name,
                default_branch,
                local_path,
                is_cloned,
            })
        })
        .collect();

    Ok(forks)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let tool_home = get_tool_home(args.tool_home)?;

    println!("Fetching your GitHub forks...");
    let forks = fetch_forks(&tool_home)?;

    if forks.is_empty() {
        println!("No forks found.");
        return Ok(());
    }

    println!(
        "Found {} forks. Tool home: {}",
        forks.len(),
        tool_home.display()
    );
    println!("Launching TUI...");

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(forks, args.dry_run, tool_home);

    // Skip to syncing if --yes flag is set
    if args.yes {
        for s in &mut app.selected {
            *s = true;
        }
        app.mark_selected_as_pending();
        app.mode = Mode::Syncing;
    }

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err:?}");
    }

    // Print summary
    let (synced, skipped, failed) = app.summary();
    if synced > 0 || skipped > 0 || failed > 0 {
        println!("\nSummary:");
        if synced > 0 {
            println!("  Synced: {synced}");
        }
        if skipped > 0 {
            println!("  Skipped: {skipped}");
        }
        if failed > 0 {
            println!("  Failed: {failed}");
        }
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let (tx, rx) = mpsc::channel::<SyncResult>();

    // Start syncing if mode is already Syncing (from --yes flag)
    if app.mode == Mode::Syncing {
        start_syncing(app, tx.clone());
    }

    loop {
        app.tick_spinner();

        // Check for sync results
        while let Ok(result) = rx.try_recv() {
            match result {
                SyncResult::StatusUpdate(idx, status) => {
                    app.statuses[idx] = status;
                }
            }
            if app.is_all_done() && app.mode == Mode::Syncing {
                app.mode = Mode::Done;
            }
        }

        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.mode {
                    Mode::Selecting => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Down | KeyCode::Char('j') => app.next(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous(),
                        KeyCode::Char(' ') | KeyCode::Tab => app.toggle_selection(),
                        KeyCode::Char('a') => app.select_all(),
                        KeyCode::Enter => {
                            if app.selected_count() > 0 {
                                app.mode = Mode::ConfirmModal;
                            }
                        }
                        _ => {}
                    },
                    Mode::ConfirmModal => match key.code {
                        KeyCode::Left | KeyCode::Char('h') => {
                            app.modal_button = 0;
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            app.modal_button = 1;
                        }
                        KeyCode::Tab => {
                            app.modal_button = 1 - app.modal_button;
                        }
                        KeyCode::Enter => {
                            if app.modal_button == 1 {
                                app.mark_selected_as_pending();
                                app.mode = Mode::Syncing;
                                start_syncing(app, tx.clone());
                            } else {
                                app.mode = Mode::Selecting;
                            }
                        }
                        KeyCode::Char('y') => {
                            app.mark_selected_as_pending();
                            app.mode = Mode::Syncing;
                            start_syncing(app, tx.clone());
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.mode = Mode::Selecting;
                        }
                        _ => {}
                    },
                    Mode::Syncing => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Down | KeyCode::Char('j') => app.next(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous(),
                        _ => {}
                    },
                    Mode::Done => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => return Ok(()),
                        KeyCode::Char('r') => {
                            app.reset_for_next_round();
                            app.mode = Mode::Selecting;
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.next(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous(),
                        _ => {}
                    },
                }
            }
        }
    }
}

fn start_syncing(app: &App, tx: mpsc::Sender<SyncResult>) {
    let forks_to_sync: Vec<(usize, Fork)> = app
        .forks
        .iter()
        .enumerate()
        .filter(|(i, _)| app.selected[*i])
        .map(|(i, f)| (i, f.clone()))
        .collect();

    let dry_run = app.dry_run;

    thread::spawn(move || {
        for (idx, fork) in forks_to_sync {
            sync_single_fork(idx, &fork, dry_run, &tx);
            thread::sleep(Duration::from_millis(100));
        }
    });
}

fn sync_single_fork(idx: usize, fork: &Fork, dry_run: bool, tx: &mpsc::Sender<SyncResult>) {
    let send = |status: SyncStatus| {
        let _ = tx.send(SyncResult::StatusUpdate(idx, status));
    };

    send(SyncStatus::Checking);

    if dry_run {
        thread::sleep(Duration::from_millis(500));
        send(SyncStatus::Synced);
        return;
    }

    // Check if repo exists locally
    if !fork.local_path.exists() {
        // Clone the repo
        send(SyncStatus::Cloning);

        // Ensure parent directory exists
        if let Some(parent) = fork.local_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                send(SyncStatus::Failed(format!("mkdir: {e}")));
                return;
            }
        }

        let clone_result = Command::new("gh")
            .args([
                "repo",
                "clone",
                &format!("{}/{}", fork.owner, fork.name),
                fork.local_path.to_string_lossy().as_ref(),
            ])
            .output();

        match clone_result {
            Ok(output) if output.status.success() => {
                send(SyncStatus::Synced);
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr);
                send(SyncStatus::Failed(truncate_error(&err)));
            }
            Err(e) => {
                send(SyncStatus::Failed(truncate_error(&e.to_string())));
            }
        }
        return;
    }

    // Repo exists locally - sync it
    let path_str = fork.local_path.to_string_lossy();

    // Check for uncommitted changes
    let status_output = Command::new("git")
        .args(["-C", &path_str, "status", "--porcelain"])
        .output();

    let is_dirty = match status_output {
        Ok(output) => !output.stdout.is_empty(),
        Err(e) => {
            send(SyncStatus::Failed(truncate_error(&e.to_string())));
            return;
        }
    };

    // Get current branch
    let branch_output = Command::new("git")
        .args(["-C", &path_str, "rev-parse", "--abbrev-ref", "HEAD"])
        .output();

    let original_branch = match branch_output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => {
            send(SyncStatus::Failed("get branch failed".to_string()));
            return;
        }
    };

    // Check for unpushed commits
    let unpushed = Command::new("git")
        .args([
            "-C",
            &path_str,
            "log",
            &format!("origin/{}..HEAD", fork.default_branch),
            "--oneline",
        ])
        .output();

    if let Ok(output) = unpushed {
        if !output.stdout.is_empty() {
            send(SyncStatus::Skipped("unpushed commits".to_string()));
            return;
        }
    }

    // Stash if dirty
    let mut stashed = false;
    if is_dirty {
        send(SyncStatus::Stashing);
        let stash_result = Command::new("git")
            .args([
                "-C",
                &path_str,
                "stash",
                "push",
                "-m",
                "repo-syncer auto-stash",
            ])
            .output();

        match stash_result {
            Ok(output) if output.status.success() => {
                stashed = true;
            }
            _ => {
                send(SyncStatus::Failed("stash failed".to_string()));
                return;
            }
        }
    }

    // Checkout default branch if not on it
    let on_default_branch = original_branch == fork.default_branch;
    if !on_default_branch {
        let checkout_result = Command::new("git")
            .args(["-C", &path_str, "checkout", &fork.default_branch])
            .output();

        if checkout_result.is_err() || !checkout_result.unwrap().status.success() {
            // Try to restore state
            if stashed {
                let _ = Command::new("git")
                    .args(["-C", &path_str, "stash", "pop"])
                    .output();
            }
            send(SyncStatus::Failed("checkout failed".to_string()));
            return;
        }
    }

    // Sync with upstream using gh repo sync
    send(SyncStatus::Syncing);
    let sync_result = Command::new("gh")
        .args([
            "repo",
            "sync",
            &format!("{}/{}", fork.owner, fork.name),
            "--source",
            &format!("{}/{}", fork.parent_owner, fork.parent_name),
            "--branch",
            &fork.default_branch,
        ])
        .output();

    let sync_success = match sync_result {
        Ok(output) => output.status.success(),
        Err(_) => false,
    };

    if !sync_success {
        // Try to restore state
        if !on_default_branch {
            let _ = Command::new("git")
                .args(["-C", &path_str, "checkout", &original_branch])
                .output();
        }
        if stashed {
            let _ = Command::new("git")
                .args(["-C", &path_str, "stash", "pop"])
                .output();
        }
        send(SyncStatus::Failed("sync failed".to_string()));
        return;
    }

    // Pull the changes locally
    send(SyncStatus::Fetching);
    let pull_result = Command::new("git")
        .args(["-C", &path_str, "pull", "--ff-only"])
        .output();

    if pull_result.is_err() || !pull_result.unwrap().status.success() {
        // Try fetch + reset instead
        let _ = Command::new("git")
            .args(["-C", &path_str, "fetch", "origin"])
            .output();
        let _ = Command::new("git")
            .args([
                "-C",
                &path_str,
                "reset",
                "--hard",
                &format!("origin/{}", fork.default_branch),
            ])
            .output();
    }

    // Restore original branch if we changed it
    if !on_default_branch {
        send(SyncStatus::Restoring);
        let _ = Command::new("git")
            .args(["-C", &path_str, "checkout", &original_branch])
            .output();
    }

    // Pop stash if we stashed
    if stashed {
        send(SyncStatus::Restoring);
        let _ = Command::new("git")
            .args(["-C", &path_str, "stash", "pop"])
            .output();
    }

    send(SyncStatus::Synced);
}

fn truncate_error(err: &str) -> String {
    let cleaned = err.trim().lines().next().unwrap_or(err);
    if cleaned.len() > 30 {
        format!("{}...", &cleaned[..27])
    } else {
        cleaned.to_string()
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Title
    let title = match app.mode {
        Mode::Selecting | Mode::ConfirmModal => {
            format!(
                " Repo Syncer {} ({} selected) ",
                if app.dry_run { "[DRY RUN]" } else { "" },
                app.selected_count()
            )
        }
        Mode::Syncing => {
            let (synced, skipped, failed) = app.summary();
            let done = synced + skipped + failed;
            let total = app.selected_count();
            format!(
                " Syncing {} ({}/{}) ",
                if app.dry_run { "[DRY RUN]" } else { "" },
                done,
                total
            )
        }
        Mode::Done => {
            let (synced, skipped, failed) = app.summary();
            format!(" Done! Synced: {synced} | Skipped: {skipped} | Failed: {failed} ")
        }
    };

    let title_block = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).bold())
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title_block, chunks[0]);

    // Table
    let header_cells = ["St", "Repository", "Local Path", "Branch", "Status"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).bold()));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.forks.iter().enumerate().map(|(i, fork)| {
        let status_icon = match &app.statuses[i] {
            SyncStatus::Pending => {
                if app.selected[i] {
                    Cell::from("*").style(Style::default().fg(Color::Green))
                } else {
                    Cell::from(" ")
                }
            }
            SyncStatus::Checking
            | SyncStatus::Cloning
            | SyncStatus::Stashing
            | SyncStatus::Fetching
            | SyncStatus::Syncing
            | SyncStatus::Restoring => {
                Cell::from(app.spinner()).style(Style::default().fg(Color::Cyan))
            }
            SyncStatus::Synced => Cell::from("v").style(Style::default().fg(Color::Green)),
            SyncStatus::Skipped(_) => Cell::from("-").style(Style::default().fg(Color::Yellow)),
            SyncStatus::Failed(_) => Cell::from("x").style(Style::default().fg(Color::Red)),
        };

        let local_path_display = fork.local_path.strip_prefix(&app.tool_home).map_or_else(
            |_| fork.local_path.display().to_string(),
            |p| format!("~/{}", p.display()),
        );

        let cloned_indicator = if fork.is_cloned { "" } else { " (new)" };

        let style = match &app.statuses[i] {
            SyncStatus::Synced => Style::default().fg(Color::Green),
            SyncStatus::Skipped(_) => Style::default().fg(Color::Yellow),
            SyncStatus::Failed(_) => Style::default().fg(Color::Red),
            SyncStatus::Checking
            | SyncStatus::Cloning
            | SyncStatus::Stashing
            | SyncStatus::Fetching
            | SyncStatus::Syncing
            | SyncStatus::Restoring => Style::default().fg(Color::Cyan),
            SyncStatus::Pending if app.selected[i] => Style::default().fg(Color::White),
            SyncStatus::Pending => Style::default().fg(Color::DarkGray),
        };

        Row::new(vec![
            status_icon,
            Cell::from(format!("{}/{}", fork.parent_owner, fork.name)),
            Cell::from(format!("{local_path_display}{cloned_indicator}")),
            Cell::from(fork.default_branch.clone()),
            Cell::from(app.statuses[i].display().to_string()),
        ])
        .style(style)
        .height(1)
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(35),
            Constraint::Length(35),
            Constraint::Length(10),
            Constraint::Min(15),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Forks "))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol("> ");

    f.render_stateful_widget(table, chunks[1], &mut app.state);

    // Help bar
    let help_text = match app.mode {
        Mode::Selecting => "j/k: Navigate | Space: Toggle | a: All | Enter: Sync | q: Quit",
        Mode::ConfirmModal => "h/l or Tab: Switch | Enter: Select | Esc: Cancel",
        Mode::Syncing => "j/k: Scroll | q: Quit",
        Mode::Done => "r: Reset and continue | q: Quit",
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);

    // Confirmation modal
    if app.mode == Mode::ConfirmModal {
        render_modal(f, app);
    }
}

fn render_modal(f: &mut Frame, app: &App) {
    let area = f.area();

    let modal_width = 50;
    let modal_height = 9;
    let modal_area = Rect {
        x: area.width.saturating_sub(modal_width) / 2,
        y: area.height.saturating_sub(modal_height) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let count = app.selected_count();
    let not_cloned = app
        .forks
        .iter()
        .enumerate()
        .filter(|(i, f)| app.selected[*i] && !f.is_cloned)
        .count();

    let (cancel_style, proceed_style) = if app.modal_button == 0 {
        (
            Style::default().fg(Color::Black).bg(Color::White).bold(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (
            Style::default().fg(Color::DarkGray),
            Style::default().fg(Color::Black).bg(Color::Green).bold(),
        )
    };

    let buttons = Line::from(vec![
        Span::styled(" [ CANCEL ] ", cancel_style),
        Span::raw("     "),
        Span::styled(" [ PROCEED ] ", proceed_style),
    ]);

    let clone_info = if not_cloned > 0 {
        format!("({not_cloned} will be cloned)")
    } else {
        String::new()
    };

    let text = vec![
        Line::from(""),
        Line::from(format!(
            "Sync {} fork{}? {clone_info}",
            count,
            if count == 1 { "" } else { "s" }
        ))
        .style(Style::default().bold())
        .centered(),
        Line::from(""),
        Line::from(if app.dry_run {
            "(Dry run - no changes will be made)"
        } else {
            "This will update forks with upstream changes."
        })
        .style(Style::default().fg(if app.dry_run {
            Color::Yellow
        } else {
            Color::Cyan
        }))
        .centered(),
        Line::from(""),
        buttons.centered(),
        Line::from(""),
        Line::from("h/l: Switch | Enter: Select | Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray))
            .centered(),
    ];

    let modal = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Confirm "),
    );

    f.render_widget(modal, modal_area);
}

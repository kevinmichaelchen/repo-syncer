mod app;
mod cache;
mod cli;
mod github;
mod sync;
mod types;
mod ui;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::{env, io, sync::mpsc, thread, time::Duration};

use app::App;
use cache::Cache;
use cli::Args;
use github::fetch_forks_graphql;
use sync::{archive_fork_async, clone_fork_async, start_syncing};
use types::{CacheStatus, Fork, ModalAction, Mode, SyncResult};

fn main() -> Result<()> {
    let args = Args::parse();
    let tool_home = get_tool_home(args.tool_home.clone())?;

    // Try to load from cache first
    let cache = Cache::open().ok();
    let (forks, cache_status) = load_forks_with_cache(cache.as_ref(), &tool_home, args.refresh)?;

    if forks.is_empty() {
        println!("No forks found.");
        return Ok(());
    }

    let cloned_count = forks.iter().filter(|f| f.is_cloned).count();
    let uncloned_count = forks.len() - cloned_count;
    let cache_msg = match cache_status {
        CacheStatus::Fresh => "(cached)",
        CacheStatus::Stale { refreshing: true } => "(refreshing...)",
        CacheStatus::Stale { refreshing: false } => "(stale)",
        CacheStatus::Offline => "(offline)",
    };
    println!(
        "Found {} forks ({} cloned, {} uncloned) {} Tool home: {}",
        forks.len(),
        cloned_count,
        uncloned_count,
        cache_msg,
        tool_home.display()
    );
    println!("Launching TUI...");

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(forks, args.dry_run, tool_home.clone(), cache_status);

    // Skip to syncing if --yes flag is set (only sync cloned forks)
    if args.yes {
        for (i, fork) in app.forks.iter().enumerate() {
            if fork.is_cloned {
                app.selected[i] = true;
            }
        }
        if app.selected_count() > 0 {
            app.mark_selected_as_pending();
            app.mode = Mode::Syncing;
        }
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

fn get_tool_home(args_tool_home: Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(path) = args_tool_home {
        return Ok(path);
    }
    let home = env::var("HOME").context("HOME environment variable not set")?;
    Ok(std::path::PathBuf::from(home).join("dev"))
}

/// Load forks with cache support.
/// Returns (forks, `cache_status`) tuple.
fn load_forks_with_cache(
    cache: Option<&Cache>,
    tool_home: &std::path::Path,
    force_refresh: bool,
) -> Result<(Vec<Fork>, CacheStatus)> {
    // If no cache available, fetch directly
    let Some(cache) = cache else {
        let forks = fetch_forks_graphql(tool_home)?;
        return Ok((forks, CacheStatus::Fresh));
    };

    // Check if we should use cache or refresh
    let cache_empty = cache.is_empty().unwrap_or(true);

    if force_refresh || cache_empty {
        // Fetch fresh data from GitHub
        match fetch_forks_graphql(tool_home) {
            Ok(forks) => {
                // Save to cache
                if let Err(e) = cache.save_forks(&forks) {
                    eprintln!("Warning: Failed to save to cache: {e}");
                }
                if let Err(e) = cache.set_last_full_sync(Utc::now()) {
                    eprintln!("Warning: Failed to update last sync time: {e}");
                }
                Ok((forks, CacheStatus::Fresh))
            }
            Err(e) => {
                // If fetch failed but we have cache, use it
                if cache_empty {
                    Err(e)
                } else {
                    eprintln!("Warning: GitHub fetch failed, using cache: {e}");
                    let forks = cache.load_forks(tool_home)?;
                    Ok((forks, CacheStatus::Offline))
                }
            }
        }
    } else {
        // Load from cache
        let forks = cache.load_forks(tool_home)?;

        // Check if cache is stale (older than 24 hours)
        let is_stale = cache
            .last_full_sync()
            .ok()
            .flatten()
            .is_none_or(|last_sync| {
                let age = Utc::now() - last_sync;
                age.num_hours() >= 24
            });

        let cache_status = if is_stale {
            CacheStatus::Stale { refreshing: false }
        } else {
            CacheStatus::Fresh
        };

        Ok((forks, cache_status))
    }
}

/// Start a background refresh from GitHub.
fn start_background_refresh(
    tool_home: std::path::PathBuf,
    cache: Option<Cache>,
    tx: mpsc::Sender<SyncResult>,
) {
    thread::spawn(move || {
        match fetch_forks_graphql(&tool_home) {
            Ok(forks) => {
                // Save to cache
                if let Some(cache) = &cache {
                    if let Err(e) = cache.save_forks(&forks) {
                        eprintln!("Warning: Failed to save to cache: {e}");
                    }
                    if let Err(e) = cache.set_last_full_sync(Utc::now()) {
                        eprintln!("Warning: Failed to update last sync time: {e}");
                    }
                }
                let _ = tx.send(SyncResult::ForksRefreshed(forks));
            }
            Err(e) => {
                let _ = tx.send(SyncResult::RefreshFailed(e.to_string()));
            }
        }
    });
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let (tx, rx) = mpsc::channel::<SyncResult>();

    // Start syncing if mode is already Syncing (from --yes flag)
    if app.mode == Mode::Syncing {
        let forks_to_sync = app.forks_to_sync();
        start_syncing(forks_to_sync, app.dry_run, tx.clone());
    }

    loop {
        app.tick_spinner();

        // Check for sync results
        while let Ok(result) = rx.try_recv() {
            match result {
                SyncResult::StatusUpdate(idx, status) => {
                    if idx < app.statuses.len() {
                        app.statuses[idx] = status;
                    }
                }
                SyncResult::ForkCloned(idx) => {
                    if idx < app.forks.len() {
                        app.forks[idx].is_cloned = true;
                    }
                }
                SyncResult::ForkArchived(idx) => {
                    app.remove_fork(idx);
                    app.show_message("Fork archived!");
                }
                SyncResult::ForksRefreshed(new_forks) => {
                    // Update forks list from background refresh
                    let len = new_forks.len();
                    app.forks = new_forks;
                    app.statuses = vec![types::SyncStatus::Pending; len];
                    app.selected = vec![false; len];
                    app.update_search();
                    app.cache_status = CacheStatus::Fresh;
                    app.show_message("Forks refreshed!");
                }
                SyncResult::RefreshFailed(err) => {
                    app.show_message(&format!("Refresh failed: {err}"));
                }
            }
            if app.is_all_done() && app.mode == Mode::Syncing {
                app.mode = Mode::Done;
            }
        }

        terminal.draw(|f| ui::render(f, app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match &app.mode {
                    Mode::Selecting => {
                        if let Some(action) = handle_selecting_mode(app, key.code, &tx)? {
                            return action;
                        }
                    }
                    Mode::Search => handle_search_mode(app, key.code),
                    Mode::StatsOverlay => {
                        if matches!(key.code, KeyCode::Char('d' | 'q') | KeyCode::Esc) {
                            app.mode = Mode::Selecting;
                        }
                    }
                    Mode::ConfirmModal => handle_confirm_modal(app, key.code, &tx),
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

fn handle_selecting_mode(
    app: &mut App,
    key: KeyCode,
    tx: &mpsc::Sender<SyncResult>,
) -> Result<Option<Result<()>>> {
    match key {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(Some(Ok(()))),
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        KeyCode::Char(' ') | KeyCode::Tab => app.toggle_selection(),
        KeyCode::Char('a') => app.select_all(),
        KeyCode::Enter => {
            if app.selected_count() > 0 {
                app.modal_action = ModalAction::Sync;
                app.mode = Mode::ConfirmModal;
            }
        }
        KeyCode::Char('/') => {
            app.search_query.clear();
            app.mode = Mode::Search;
        }
        KeyCode::Char('d') => {
            app.compute_stats();
            app.mode = Mode::StatsOverlay;
        }
        KeyCode::Char('c') => {
            if let Some(fork) = app.current_fork() {
                if fork.is_cloned {
                    app.show_message("Already cloned");
                } else {
                    app.modal_action = ModalAction::Clone;
                    app.mode = Mode::ConfirmModal;
                }
            }
        }
        KeyCode::Char('o') => {
            if let Some(fork) = app.current_fork() {
                let repo = format!("{}/{}", fork.owner, fork.name);
                let _ = std::process::Command::new("gh")
                    .args(["browse", "--repo", &repo])
                    .spawn();
                app.show_message("Opening in browser...");
            }
        }
        KeyCode::Char('e') => {
            if let Some(fork) = app.current_fork() {
                if fork.is_cloned {
                    let path = fork.local_path.clone();
                    // Temporarily exit TUI
                    disable_raw_mode()?;
                    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

                    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
                    let _ = std::process::Command::new(&editor).arg(&path).status();

                    // Restore TUI
                    enable_raw_mode()?;
                    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                } else {
                    app.show_message("Not cloned yet");
                }
            }
        }
        KeyCode::Char('x') | KeyCode::Delete => {
            if app.current_fork().is_some() {
                app.modal_action = ModalAction::Archive;
                app.mode = Mode::ConfirmModal;
            }
        }
        KeyCode::Char('R') => {
            // Start background refresh from GitHub
            app.cache_status = CacheStatus::Stale { refreshing: true };
            app.show_message("Refreshing from GitHub...");
            let cache = Cache::open().ok();
            start_background_refresh(app.tool_home.clone(), cache, tx.clone());
        }
        _ => {}
    }
    Ok(None)
}

fn handle_search_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            app.search_query.clear();
            app.update_search();
            app.mode = Mode::Selecting;
        }
        KeyCode::Enter => {
            app.mode = Mode::Selecting;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.update_search();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.update_search();
        }
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        _ => {}
    }
}

fn handle_confirm_modal(app: &mut App, key: KeyCode, tx: &mpsc::Sender<SyncResult>) {
    match key {
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
                execute_modal_action(app, tx);
            } else {
                app.mode = Mode::Selecting;
            }
        }
        KeyCode::Char('y') => {
            app.modal_button = 1;
            execute_modal_action(app, tx);
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.mode = Mode::Selecting;
        }
        _ => {}
    }
}

fn execute_modal_action(app: &mut App, tx: &mpsc::Sender<SyncResult>) {
    match app.modal_action {
        ModalAction::Sync => {
            app.mark_selected_as_pending();
            app.mode = Mode::Syncing;
            let forks_to_sync = app.forks_to_sync();
            start_syncing(forks_to_sync, app.dry_run, tx.clone());
        }
        ModalAction::Clone => {
            if let Some(idx) = app.current_fork_index() {
                let fork = app.forks[idx].clone();
                app.statuses[idx] = types::SyncStatus::Cloning;
                app.selected[idx] = true;
                clone_fork_async(idx, fork, app.dry_run, tx.clone());
            }
            app.mode = Mode::Selecting;
        }
        ModalAction::Archive => {
            if let Some(idx) = app.current_fork_index() {
                let fork = app.forks[idx].clone();
                app.statuses[idx] = types::SyncStatus::Archiving;
                archive_fork_async(idx, fork, app.dry_run, tx.clone());
            }
            app.mode = Mode::Selecting;
        }
    }
}

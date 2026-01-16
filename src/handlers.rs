use crate::app::App;
use crate::cache::SqliteStore;
use crate::github::fetch_forks_graphql;
use crate::sync::{archive_fork_async, clone_fork_async, delete_fork_async, start_syncing};
use crate::types::{CacheStatus, ForkStore, ModalAction, Mode, SyncResult};
use anyhow::Result;
use chrono::Utc;
use crossterm::{
    event::KeyCode,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use std::{env, io, sync::mpsc, thread};

/// Start a background refresh from GitHub.
pub fn start_background_refresh(
    tool_home: std::path::PathBuf,
    cache: Option<SqliteStore>,
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

pub fn handle_selecting_mode(
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
            } else if let Some(idx) = app.current_fork_index() {
                // Nothing selected - sync current fork (works for both cloned and uncloned)
                app.selected[idx] = true;
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
        KeyCode::Char('x') => {
            if app.current_fork().is_some() {
                app.modal_action = ModalAction::Archive;
                app.mode = Mode::ConfirmModal;
            }
        }
        KeyCode::Char('D') => {
            if app.current_fork().is_some() {
                app.modal_action = ModalAction::Delete;
                app.mode = Mode::ConfirmModal;
            }
        }
        KeyCode::Char('R') => {
            // Start background refresh from GitHub
            app.cache_status = CacheStatus::Stale { refreshing: true };
            app.show_message("Refreshing from GitHub...");
            let cache = SqliteStore::open().ok();
            start_background_refresh(app.tool_home.clone(), cache, tx.clone());
        }
        _ => {}
    }
    Ok(None)
}

pub fn handle_search_mode(app: &mut App, key: KeyCode) {
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

pub fn handle_error_popup(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Char('q' | 'n') => {
            app.dismiss_error_popup();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.modal_button = 0; // Action button
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.modal_button = 1; // Dismiss button
        }
        KeyCode::Tab => {
            app.modal_button = 1 - app.modal_button;
        }
        KeyCode::Enter | KeyCode::Char('y') => {
            // Execute based on selected button
            if app.modal_button == 0 {
                // Run the action if available
                if let Some(details) = &app.error_details {
                    if let Some(action) = &details.action {
                        let command = action.command.clone();
                        app.dismiss_error_popup();
                        std::thread::spawn(move || {
                            let _ = std::process::Command::new("sh")
                                .arg("-c")
                                .arg(&command)
                                .status();
                        });
                        app.show_message("Running fix command...");
                    } else {
                        app.dismiss_error_popup();
                    }
                } else {
                    app.dismiss_error_popup();
                }
            } else {
                // Dismiss
                app.dismiss_error_popup();
            }
        }
        _ => {}
    }
}

pub fn handle_confirm_modal(app: &mut App, key: KeyCode, tx: &mpsc::Sender<SyncResult>) {
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

pub fn execute_modal_action(app: &mut App, tx: &mpsc::Sender<SyncResult>) {
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
                app.statuses[idx] = crate::types::SyncStatus::Cloning;
                app.selected[idx] = true;
                clone_fork_async(idx, fork, app.dry_run, tx.clone());
            }
            app.mode = Mode::Selecting;
        }
        ModalAction::Archive => {
            if let Some(idx) = app.current_fork_index() {
                let fork = app.forks[idx].clone();
                app.statuses[idx] = crate::types::SyncStatus::Archiving;
                archive_fork_async(idx, fork, app.dry_run, tx.clone());
            }
            app.mode = Mode::Selecting;
        }
        ModalAction::Delete => {
            if let Some(idx) = app.current_fork_index() {
                let fork = app.forks[idx].clone();
                app.statuses[idx] = crate::types::SyncStatus::Deleting;
                delete_fork_async(idx, fork, app.dry_run, tx.clone());
            }
            app.mode = Mode::Selecting;
        }
    }
}

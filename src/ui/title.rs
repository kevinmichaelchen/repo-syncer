use crate::app::App;
use crate::types::{CacheStatus, Mode};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub fn render_title(f: &mut Frame, app: &App, area: Rect) {
    let cache_indicator = match &app.cache_status {
        CacheStatus::Fresh => "",
        CacheStatus::Stale { refreshing: true } => " (refreshing...)",
        CacheStatus::Stale { refreshing: false } => " (cached)",
        CacheStatus::Offline => " (offline)",
    };

    let title = match app.mode {
        Mode::Selecting
        | Mode::ConfirmModal
        | Mode::Search
        | Mode::StatsOverlay
        | Mode::ErrorPopup => {
            let cloned = app.forks.iter().filter(|f| f.is_cloned).count();
            let uncloned = app.forks.len() - cloned;
            format!(
                " Repo Syncer {} | {} forks ({} cloned, {} uncloned) | {} selected{cache_indicator} ",
                if app.dry_run { "[DRY RUN]" } else { "" },
                app.forks.len(),
                cloned,
                uncloned,
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
    };

    let title_block = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).bold())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
    f.render_widget(title_block, area);
}

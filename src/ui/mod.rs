mod details;
mod help;
mod list;
mod overlays;
mod search;
mod title;

use crate::app::App;
use crate::types::Mode;
use ratatui::prelude::*;

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Determine if we show details pane (need at least 100 chars width)
    let show_details = area.width >= 100;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    // Title
    title::render_title(f, app, main_chunks[0]);

    // Main content area - split horizontally if wide enough
    let content_area = main_chunks[1];
    let (list_area, details_area) = if show_details {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(content_area);
        (h_chunks[0], Some(h_chunks[1]))
    } else {
        (content_area, None)
    };

    // Render fork list
    list::render_fork_list(f, app, list_area);

    // Render details pane if visible
    if let Some(details) = details_area {
        details::render_details_pane(f, app, details);
    }

    // Help bar or search input
    if app.mode == Mode::Search {
        search::render_search_input(f, app, main_chunks[2]);
    } else {
        help::render_help_bar(f, app, main_chunks[2]);
    }

    // Overlays
    if app.mode == Mode::ConfirmModal {
        overlays::render_modal(f, app);
    }

    if app.mode == Mode::StatsOverlay {
        overlays::render_stats_overlay(f, app);
    }

    if app.mode == Mode::ErrorPopup {
        overlays::render_error_popup(f, app);
    }

    // Toast notifications (always on top)
    overlays::render_toasts(f, app);
}

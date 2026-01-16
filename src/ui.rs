use crate::app::App;
use crate::types::{CacheStatus, ModalAction, Mode, SyncStatus};
use ratatui::{
    prelude::*,
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

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
    render_title(f, app, main_chunks[0]);

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
    render_fork_list(f, app, list_area);

    // Render details pane if visible
    if let Some(details) = details_area {
        render_details_pane(f, app, details);
    }

    // Help bar or search input
    if app.mode == Mode::Search {
        render_search_input(f, app, main_chunks[2]);
    } else {
        render_help_bar(f, app, main_chunks[2]);
    }

    // Overlays
    if app.mode == Mode::ConfirmModal {
        render_modal(f, app);
    }

    if app.mode == Mode::StatsOverlay {
        render_stats_overlay(f, app);
    }
}

fn render_title(f: &mut Frame, app: &App, area: Rect) {
    let cache_indicator = match &app.cache_status {
        CacheStatus::Fresh => "",
        CacheStatus::Stale { refreshing: true } => " (refreshing...)",
        CacheStatus::Stale { refreshing: false } => " (cached)",
        CacheStatus::Offline => " (offline)",
    };

    let title = match app.mode {
        Mode::Selecting | Mode::ConfirmModal | Mode::Search | Mode::StatsOverlay => {
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
        Mode::Done => {
            let (synced, skipped, failed) = app.summary();
            format!(" Done! Synced: {synced} | Skipped: {skipped} | Failed: {failed} ")
        }
    };

    let title_block = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).bold())
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title_block, area);
}

fn render_fork_list(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["St", "Repository", "Status"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).bold()));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let visible = app.visible_forks();
    let rows = visible.iter().map(|&i| {
        let fork = &app.forks[i];
        let status_icon = match &app.statuses[i] {
            SyncStatus::Pending => {
                if app.selected[i] {
                    Cell::from("*").style(Style::default().fg(Color::Green))
                } else if fork.is_cloned {
                    Cell::from(" ")
                } else {
                    Cell::from("○").style(Style::default().fg(Color::DarkGray))
                }
            }
            SyncStatus::Checking
            | SyncStatus::Cloning
            | SyncStatus::Stashing
            | SyncStatus::Fetching
            | SyncStatus::Syncing
            | SyncStatus::Restoring
            | SyncStatus::Archiving => {
                Cell::from(app.spinner()).style(Style::default().fg(Color::Cyan))
            }
            SyncStatus::Synced => Cell::from("✓").style(Style::default().fg(Color::Green)),
            SyncStatus::Skipped(_) => Cell::from("-").style(Style::default().fg(Color::Yellow)),
            SyncStatus::Failed(_) => Cell::from("✗").style(Style::default().fg(Color::Red)),
        };

        let repo_name = format!("{}/{}", fork.parent_owner, fork.name);
        let cloned_indicator = if fork.is_cloned { "" } else { " (uncloned)" };

        let style = match &app.statuses[i] {
            SyncStatus::Synced => Style::default().fg(Color::Green),
            SyncStatus::Skipped(_) => Style::default().fg(Color::Yellow),
            SyncStatus::Failed(_) => Style::default().fg(Color::Red),
            SyncStatus::Checking
            | SyncStatus::Cloning
            | SyncStatus::Stashing
            | SyncStatus::Fetching
            | SyncStatus::Syncing
            | SyncStatus::Restoring
            | SyncStatus::Archiving => Style::default().fg(Color::Cyan),
            SyncStatus::Pending if app.selected[i] => Style::default().fg(Color::White),
            SyncStatus::Pending if !fork.is_cloned => Style::default().fg(Color::DarkGray),
            SyncStatus::Pending => Style::default().fg(Color::Reset),
        };

        Row::new(vec![
            status_icon,
            Cell::from(format!("{repo_name}{cloned_indicator}")),
            Cell::from(app.statuses[i].display().to_string()),
        ])
        .style(style)
        .height(1)
    });

    let title = if app.search_query.is_empty() {
        " Forks ".to_string()
    } else {
        format!(" Forks ({} matches) ", visible.len())
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Min(30),
            Constraint::Length(20),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol("> ");

    f.render_stateful_widget(table, area, &mut app.state);
}

fn render_details_pane(f: &mut Frame, app: &App, area: Rect) {
    let fork = app.current_fork();

    let content = if let Some(fork) = fork {
        let local_path_display = fork.local_path.strip_prefix(&app.tool_home).map_or_else(
            |_| fork.local_path.display().to_string(),
            |p| format!("~/{}", p.display()),
        );

        let description = fork
            .description
            .as_deref()
            .unwrap_or("No description")
            .chars()
            .take(200)
            .collect::<String>();

        let language = fork.primary_language.as_deref().unwrap_or("Unknown");
        let clone_status = if fork.is_cloned {
            "Cloned"
        } else {
            "Not cloned"
        };

        vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}/{}", fork.owner, fork.name),
                    Style::default().fg(Color::Cyan).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Parent: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}/{}", fork.parent_owner, fork.parent_name),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Description: ",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(Span::styled(description, Style::default().fg(Color::White))),
            Line::from(""),
            Line::from(vec![
                Span::styled("Language: ", Style::default().fg(Color::DarkGray)),
                Span::styled(language, Style::default().fg(Color::Magenta)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Branch: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&fork.default_branch, Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    clone_status,
                    Style::default().fg(if fork.is_cloned {
                        Color::Green
                    } else {
                        Color::Yellow
                    }),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                Span::styled(local_path_display, Style::default().fg(Color::Blue)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "No fork selected",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let details = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(" Details "))
        .wrap(ratatui::widgets::Wrap { trim: true });

    f.render_widget(details, area);
}

fn render_search_input(f: &mut Frame, app: &App, area: Rect) {
    let input = Paragraph::new(format!("Search: {}_", app.search_query))
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(" Filter "));
    f.render_widget(input, area);
}

fn render_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.mode {
        Mode::Selecting => {
            if let Some((msg, _)) = &app.status_message {
                msg.as_str()
            } else {
                "j/k: Nav | Space: Select | a: All | Enter: Sync | c: Clone | o: Open | e: Edit | /: Search | d: Stats | R: Refresh | q: Quit"
            }
        }
        Mode::Search => "Type to filter | Enter: Confirm | Esc: Cancel",
        Mode::StatsOverlay => "d or Esc: Close stats",
        Mode::ConfirmModal => "h/l or Tab: Switch | Enter: Select | Esc: Cancel",
        Mode::Syncing => "j/k: Scroll | q: Quit",
        Mode::Done => "r: Reset and continue | q: Quit",
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, area);
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

    let (title, message) = match app.modal_action {
        ModalAction::Sync => {
            let count = app.selected_count();
            let not_cloned = app
                .forks
                .iter()
                .enumerate()
                .filter(|(i, f)| app.selected[*i] && !f.is_cloned)
                .count();
            let clone_info = if not_cloned > 0 {
                format!(" ({not_cloned} will be cloned)")
            } else {
                String::new()
            };
            (
                " Confirm Sync ",
                format!(
                    "Sync {} fork{}?{clone_info}",
                    count,
                    if count == 1 { "" } else { "s" }
                ),
            )
        }
        ModalAction::Clone => {
            let name = app
                .current_fork()
                .map(|f| format!("{}/{}", f.parent_owner, f.name))
                .unwrap_or_default();
            (" Confirm Clone ", format!("Clone {name}?"))
        }
        ModalAction::Archive => {
            let name = app
                .current_fork()
                .map(|f| format!("{}/{}", f.owner, f.name))
                .unwrap_or_default();
            (
                " ⚠ Archive Fork ",
                format!("Archive {name}? This cannot be undone."),
            )
        }
    };

    let (cancel_style, proceed_style) = if app.modal_button == 0 {
        (
            Style::default().fg(Color::Black).bg(Color::White).bold(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (
            Style::default().fg(Color::DarkGray),
            Style::default()
                .fg(Color::Black)
                .bg(if app.modal_action == ModalAction::Archive {
                    Color::Red
                } else {
                    Color::Green
                })
                .bold(),
        )
    };

    let buttons = Line::from(vec![
        Span::styled(" [ CANCEL ] ", cancel_style),
        Span::raw("     "),
        Span::styled(" [ PROCEED ] ", proceed_style),
    ]);

    let text = vec![
        Line::from(""),
        Line::from(message)
            .style(Style::default().bold())
            .centered(),
        Line::from(""),
        Line::from(if app.dry_run {
            "(Dry run - no changes will be made)"
        } else {
            ""
        })
        .style(Style::default().fg(Color::Yellow))
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
            .border_style(
                Style::default().fg(if app.modal_action == ModalAction::Archive {
                    Color::Red
                } else {
                    Color::Cyan
                }),
            )
            .title(title),
    );

    f.render_widget(modal, modal_area);
}

fn render_stats_overlay(f: &mut Frame, app: &App) {
    let area = f.area();

    let modal_width = 60.min(area.width.saturating_sub(4));
    let modal_height = 18.min(area.height.saturating_sub(4));
    let modal_area = Rect {
        x: area.width.saturating_sub(modal_width) / 2,
        y: area.height.saturating_sub(modal_height) / 2,
        width: modal_width,
        height: modal_height,
    };

    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Fork Statistics ");

    let inner = block.inner(modal_area);
    f.render_widget(block, modal_area);

    if let Some(stats) = &app.stats_cache {
        // Split inner area
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Summary
                Constraint::Min(5),    // Charts
            ])
            .split(inner);

        // Summary line
        let summary = format!(
            "Total: {} | Cloned: {} | Uncloned: {}",
            stats.total, stats.cloned, stats.uncloned
        );
        let summary_widget = Paragraph::new(summary)
            .style(Style::default().fg(Color::White).bold())
            .centered();
        f.render_widget(summary_widget, chunks[0]);

        // Language bar chart
        if !stats.by_language.is_empty() {
            let bars: Vec<Bar> = stats
                .by_language
                .iter()
                .map(|(lang, count)| {
                    let label = if lang.len() > 8 {
                        format!("{}…", &lang[..7])
                    } else {
                        lang.clone()
                    };
                    Bar::default()
                        .value(*count)
                        .label(Line::from(label))
                        .style(Style::default().fg(Color::Cyan))
                })
                .collect();

            let chart = BarChart::default()
                .block(Block::default().title(" Languages ").borders(Borders::TOP))
                .data(BarGroup::default().bars(&bars))
                .bar_width(8)
                .bar_gap(1)
                .value_style(Style::default().fg(Color::White).bold());

            f.render_widget(chart, chunks[1]);
        }
    }
}

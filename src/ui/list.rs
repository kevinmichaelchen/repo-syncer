use crate::app::App;
use crate::types::SyncStatus;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
};

pub fn render_fork_list(f: &mut Frame, app: &mut App, area: Rect) {
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
            | SyncStatus::Archiving
            | SyncStatus::Deleting => {
                Cell::from(app.spinner()).style(Style::default().fg(Color::Cyan))
            }
            SyncStatus::Synced(_) => Cell::from("✓").style(Style::default().fg(Color::Green)),
            SyncStatus::Skipped(_) => Cell::from("-").style(Style::default().fg(Color::Yellow)),
            SyncStatus::Failed(_) => Cell::from("✗").style(Style::default().fg(Color::Red)),
        };

        let repo_name = format!("{}/{}", fork.parent_owner, fork.name);

        // Determine display status (show "Not cloned" for uncloned forks)
        let display_status = if !fork.is_cloned
            && matches!(app.statuses[i], SyncStatus::Pending | SyncStatus::Checking)
        {
            "Not cloned".to_string()
        } else {
            app.statuses[i].display()
        };

        let style = match &app.statuses[i] {
            SyncStatus::Synced(_) => Style::default().fg(Color::Green),
            SyncStatus::Skipped(_) => Style::default().fg(Color::Yellow),
            SyncStatus::Failed(_) => Style::default().fg(Color::Red),
            SyncStatus::Checking
            | SyncStatus::Cloning
            | SyncStatus::Stashing
            | SyncStatus::Fetching
            | SyncStatus::Syncing
            | SyncStatus::Restoring
            | SyncStatus::Archiving
            | SyncStatus::Deleting => Style::default().fg(Color::Cyan),
            SyncStatus::Pending if app.selected[i] => Style::default().fg(Color::White).bold(),
            SyncStatus::Pending if !fork.is_cloned => Style::default().fg(Color::DarkGray).dim(),
            SyncStatus::Pending => Style::default().fg(Color::Reset),
        };

        Row::new(vec![
            status_icon,
            Cell::from(repo_name),
            Cell::from(display_status),
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
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut app.state);
}

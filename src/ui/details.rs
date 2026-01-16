use crate::app::App;
use chrono::{DateTime, Utc};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

pub fn render_details_pane(f: &mut Frame, app: &App, area: Rect) {
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

        let forked_date = fork
            .created_at
            .map_or_else(|| "Unknown".to_string(), format_relative_date);

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
                Span::styled("Forked: ", Style::default().fg(Color::DarkGray)),
                Span::styled(forked_date, Style::default().fg(Color::Cyan)),
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Details "),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(details, area);
}

/// Format a date as relative time (e.g., "3 months ago") with actual date
fn format_relative_date(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    let relative = if duration.num_days() < 1 {
        "today".to_string()
    } else if duration.num_days() == 1 {
        "yesterday".to_string()
    } else if duration.num_days() < 7 {
        format!("{} days ago", duration.num_days())
    } else if duration.num_weeks() < 4 {
        let weeks = duration.num_weeks();
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else if duration.num_days() < 365 {
        let months = duration.num_days() / 30;
        format!("{} month{} ago", months, if months == 1 { "" } else { "s" })
    } else {
        let years = duration.num_days() / 365;
        format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
    };

    let date_str = dt.format("%b %d, %Y").to_string();
    format!("{relative} ({date_str})")
}

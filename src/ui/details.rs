use crate::app::App;
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Details "),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(details, area);
}

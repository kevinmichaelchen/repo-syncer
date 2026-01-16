use crate::app::App;
use crate::types::{ModalAction, ToastLevel};
use ratatui::{
    prelude::*,
    widgets::{Bar, BarChart, BarGroup, Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

pub fn render_modal(f: &mut Frame, app: &App) {
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
        ModalAction::Delete => {
            let name = app
                .current_fork()
                .map(|f| format!("{}/{}", f.owner, f.name))
                .unwrap_or_default();
            let cloned = app.current_fork().is_some_and(|f| f.is_cloned);
            let extra = if cloned {
                " Local clone will also be removed."
            } else {
                ""
            };
            (
                " ⚠ DELETE Fork ",
                format!("Permanently delete {name}?{extra}"),
            )
        }
    };

    let is_destructive = matches!(app.modal_action, ModalAction::Archive | ModalAction::Delete);

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
                .bg(if is_destructive {
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
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if is_destructive {
                Color::Red
            } else {
                Color::Cyan
            }))
            .title(title),
    );

    f.render_widget(modal, modal_area);
}

pub fn render_stats_overlay(f: &mut Frame, app: &App) {
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
        .border_type(BorderType::Rounded)
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

pub fn render_toasts(f: &mut Frame, app: &App) {
    if app.toasts.is_empty() {
        return;
    }

    let area = f.area();

    // Render toasts in bottom-right corner, stacked vertically
    for (i, toast) in app.toasts.iter().enumerate() {
        let toast_width = (toast.message.len() as u16 + 6).min(60);
        let toast_height = 3;

        let x = area.width.saturating_sub(toast_width + 2);
        let y = area.height.saturating_sub((i as u16 + 1) * (toast_height + 1) + 1);

        let toast_area = Rect {
            x,
            y,
            width: toast_width,
            height: toast_height,
        };

        let (border_color, icon) = match toast.level {
            ToastLevel::Info => (Color::Cyan, "ℹ"),
            ToastLevel::Success => (Color::Green, "✓"),
            ToastLevel::Warning => (Color::Yellow, "⚠"),
            ToastLevel::Error => (Color::Red, "✗"),
        };

        f.render_widget(Clear, toast_area);

        let toast_text = format!("{icon} {}", toast.message);
        let toast_widget = Paragraph::new(toast_text)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(border_color)),
            );

        f.render_widget(toast_widget, toast_area);
    }
}

pub fn render_error_popup(f: &mut Frame, app: &App) {
    let Some(details) = &app.error_details else {
        return;
    };

    let area = f.area();

    // Calculate width based on message length, but clamp to reasonable bounds
    let content_width = details.message.lines().map(str::len).max().unwrap_or(40) as u16;
    let modal_width = (content_width + 6).clamp(40, area.width.saturating_sub(4));
    let modal_height = if details.action.is_some() { 14 } else { 10 };
    let modal_height = modal_height.min(area.height.saturating_sub(4));

    let modal_area = Rect {
        x: area.width.saturating_sub(modal_width) / 2,
        y: area.height.saturating_sub(modal_height) / 2,
        width: modal_width,
        height: modal_height,
    };

    f.render_widget(Clear, modal_area);

    let mut text = vec![
        Line::from(""),
        Line::from(Span::styled(
            &details.message,
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    if let Some(action) = &details.action {
        // Determine button styles based on selection
        let (action_style, dismiss_style) = if app.modal_button == 0 {
            (
                Style::default().fg(Color::Black).bg(Color::Green).bold(),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            (
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::Black).bg(Color::White).bold(),
            )
        };

        text.push(Line::from(""));
        text.push(
            Line::from(Span::styled(
                format!("Suggested fix: {}", action.label),
                Style::default().fg(Color::Yellow),
            ))
            .centered(),
        );
        text.push(Line::from(""));
        text.push(
            Line::from(vec![
                Span::styled(format!(" [ {} ] ", action.label), action_style),
                Span::raw("     "),
                Span::styled(" [ Dismiss ] ", dismiss_style),
            ])
            .centered(),
        );
        text.push(Line::from(""));
        text.push(
            Line::from("h/l: Switch | Enter: Select | Esc: Dismiss")
                .style(Style::default().fg(Color::DarkGray))
                .centered(),
        );
    } else {
        text.push(Line::from(""));
        text.push(
            Line::from(Span::styled(
                " [ OK ] ",
                Style::default().fg(Color::Black).bg(Color::White).bold(),
            ))
            .centered(),
        );
        text.push(Line::from(""));
        text.push(
            Line::from("Enter or Esc: Dismiss")
                .style(Style::default().fg(Color::DarkGray))
                .centered(),
        );
    }

    let modal = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Red))
                .title(format!(" ⚠ {} ", details.title)),
        );

    f.render_widget(modal, modal_area);
}

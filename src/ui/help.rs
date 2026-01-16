use crate::app::App;
use crate::types::Mode;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub fn render_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.mode {
        Mode::Selecting => {
            if let Some((msg, _)) = &app.status_message {
                msg.as_str()
            } else {
                "j/k: Nav | Space: Select | a: All | Enter: Sync | c: Clone | x: Archive | D: Delete | o: Open | /: Search | q: Quit"
            }
        }
        Mode::Search => "Type to filter | Enter: Confirm | Esc: Cancel",
        Mode::StatsOverlay => "d or Esc: Close stats",
        Mode::ConfirmModal => "h/l or Tab: Switch | Enter: Select | Esc: Cancel",
        Mode::ErrorPopup => "Enter: Run action | Esc: Dismiss",
        Mode::Syncing => "j/k: Scroll | q: Quit",
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
    f.render_widget(help, area);
}

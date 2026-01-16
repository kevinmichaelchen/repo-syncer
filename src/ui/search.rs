use crate::app::App;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub fn render_search_input(f: &mut Frame, app: &App, area: Rect) {
    let input = Paragraph::new(format!("Search: {}_", app.search_query))
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Filter "),
        );
    f.render_widget(input, area);
}

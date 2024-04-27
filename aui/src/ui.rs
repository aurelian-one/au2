use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::Direction;
use ratatui::style::{Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, List, ListDirection, Paragraph};
use crate::app::{App, Mode};

pub fn ui(f: &mut Frame, app: &App) {

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.size());

    match app.mode {
        Mode::Tree(ctx) => {

            let title = Paragraph::new(Text::styled("Items", Style::default()))
                .block(Block::default().borders(Borders::ALL).style(Style::default()));
            f.render_widget(title, chunks[0]);
            
            let core = List::new([
                "one", "two", "three",
                "one", "two", "three",
                "one", "two", "three",
                "one", "two", "three",
                "one", "two", "three",
                "one", "two", "three",
            ])
                .block(Block::default().borders(Borders::ALL).style(Style::default()))
                .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
                .highlight_symbol(">>")
                .repeat_highlight_symbol(true)
                .direction(ListDirection::TopToBottom);

            f.render_stateful_widget(core, chunks[1]);

            let footer = Paragraph::new(Text::styled("Example Footer", Style::default()))
                .block(Block::default().borders(Borders::ALL).style(Style::default()));
            f.render_widget(footer, chunks[2]);

        }
        Mode::Detail(_) => {


        }
    }

}

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, ChatMessage};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let inner_width = area.width.saturating_sub(2);
    let inner_height = area.height.saturating_sub(2) as usize;
    let sep = "─".repeat(inner_width as usize);

    let mut lines: Vec<Line> = Vec::new();

    for (i, msg) in app.chat.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(Span::styled(sep.clone(), Style::default().fg(Color::DarkGray))));
        }
        match msg {
            ChatMessage::User(text) => {
                let text_lines: Vec<&str> = if text.is_empty() { vec![""] } else { text.lines().collect() };
                let mut first = true;
                for src_line in text_lines {
                    if first {
                        lines.push(Line::from(vec![
                            Span::styled("> ", Style::default().fg(Color::Cyan)),
                            Span::raw(src_line.to_string()),
                        ]));
                        first = false;
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::raw(src_line.to_string()),
                        ]));
                    }
                }
            }
            ChatMessage::Assistant { text, .. } => {
                let text_lines: Vec<&str> = if text.is_empty() { vec![""] } else { text.lines().collect() };
                let mut first = true;
                for src_line in text_lines {
                    if first {
                        lines.push(Line::from(vec![
                            Span::styled("● ", Style::default().fg(Color::White)),
                            Span::raw(src_line.to_string()),
                        ]));
                        first = false;
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::raw(src_line.to_string()),
                        ]));
                    }
                }
            }
            ChatMessage::System(text) => {
                lines.push(Line::from(Span::styled(
                    text.clone(),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                )));
            }
        }
    }

    if !app.chat.is_empty() {
        lines.push(Line::from(Span::styled(sep, Style::default().fg(Color::DarkGray))));
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Chat (PgUp/PgDn to scroll) "))
        .wrap(Wrap { trim: false });

    let total_visual_lines = paragraph.line_count(inner_width);
    let max_scroll = total_visual_lines.saturating_sub(inner_height);
    app.chat_max_scroll = max_scroll;

    let scroll = if app.chat_pinned {
        max_scroll
    } else {
        app.chat_scroll.min(max_scroll)
    };

    f.render_widget(paragraph.scroll((scroll as u16, 0)), area);
}

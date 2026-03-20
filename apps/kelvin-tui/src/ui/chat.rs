use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, ChatMessage};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.chat {
        match msg {
            ChatMessage::User(text) => {
                let mut first = true;
                for src_line in text.lines() {
                    if first {
                        lines.push(Line::from(vec![
                            Span::styled("You: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            Span::raw(src_line.to_string()),
                        ]));
                        first = false;
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("     "),
                            Span::raw(src_line.to_string()),
                        ]));
                    }
                }
                if text.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("You: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    ]));
                }
            }
            ChatMessage::Assistant { text, .. } => {
                let mut first = true;
                for src_line in text.lines() {
                    if first {
                        lines.push(Line::from(vec![
                            Span::styled("Kelvin: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                            Span::raw(src_line.to_string()),
                        ]));
                        first = false;
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("        "),
                            Span::raw(src_line.to_string()),
                        ]));
                    }
                }
                if text.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Kelvin: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    ]));
                }
            }
            ChatMessage::System(text) => {
                lines.push(Line::from(vec![
                    Span::styled(
                        text.clone(),
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        }
        lines.push(Line::default());
    }

    let inner_width = area.width.saturating_sub(2);
    let inner_height = area.height.saturating_sub(2) as usize;

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Chat (PgUp/PgDn to scroll) "))
        .wrap(Wrap { trim: false });

    // line_count gives the exact visual line count after word-wrap at this width
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

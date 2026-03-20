use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, ChatMessage};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.chat {
        match msg {
            ChatMessage::User(text) => {
                // Split on newlines so each source line becomes a ratatui Line,
                // with the "You: " prefix only on the first.
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
        // Blank line between messages
        lines.push(Line::default());
    }

    // Inner height (excluding borders) for scroll clamping
    let inner_height = area.height.saturating_sub(2) as usize;
    // Each source line may wrap, but use line count as a rough scroll unit
    let total_lines = lines.len();
    let scroll = app.chat_scroll.min(total_lines.saturating_sub(inner_height)) as u16;

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Chat (PgUp/PgDn to scroll) "))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(paragraph, area);
}

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthChar;

use crate::app::{App, ChatLineInfo, ChatMessage, SelectionTarget};

const HL_BG: Color = Color::Indexed(238); // dark gray selection highlight

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let inner_height = area.height.saturating_sub(2) as usize;
    let sep_str = "─".repeat(inner_width);

    // --- Build content lines + per-line metadata ---
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut line_info: Vec<ChatLineInfo> = Vec::new();
    let mut line_texts: Vec<String> = Vec::new();

    for (i, msg) in app.chat.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(Span::styled(
                sep_str.clone(),
                Style::default().fg(Color::DarkGray),
            )));
            line_info.push(ChatLineInfo {
                prefix_width: inner_width,
                is_separator: true,
            });
            line_texts.push(String::new());
            // blank spacer line between messages
            lines.push(Line::from(vec![]));
            line_info.push(ChatLineInfo {
                prefix_width: 0,
                is_separator: false,
            });
            line_texts.push(String::new());
        }
        match msg {
            ChatMessage::User(text) => {
                let all_lines: Vec<&str> = text.lines().collect();
                let src_lines: Vec<&str> = if all_lines.is_empty() {
                    vec![""]
                } else {
                    let f: Vec<&str> = all_lines
                        .into_iter()
                        .filter(|l| !l.trim().is_empty())
                        .collect();
                    if f.is_empty() {
                        vec![""]
                    } else {
                        f
                    }
                };
                let mut first = true;
                for src_line in src_lines {
                    let line = if first {
                        Line::from(vec![
                            Span::styled("> ", Style::default().fg(Color::Cyan)),
                            Span::raw(src_line.to_string()),
                        ])
                    } else {
                        Line::from(vec![Span::raw("  "), Span::raw(src_line.to_string())])
                    };
                    first = false;
                    lines.push(line);
                    line_info.push(ChatLineInfo {
                        prefix_width: 2,
                        is_separator: false,
                    });
                    line_texts.push(src_line.to_string());
                }
            }
            ChatMessage::Assistant { text, .. } => {
                let all_lines: Vec<&str> = text.lines().collect();
                let src_lines: Vec<&str> = if all_lines.is_empty() {
                    vec![""]
                } else {
                    let f: Vec<&str> = all_lines
                        .into_iter()
                        .filter(|l| !l.trim().is_empty())
                        .collect();
                    if f.is_empty() {
                        vec![""]
                    } else {
                        f
                    }
                };
                let mut first = true;
                for src_line in src_lines {
                    let line = if first {
                        Line::from(vec![
                            Span::styled("● ", Style::default().fg(Color::White)),
                            Span::raw(src_line.to_string()),
                        ])
                    } else {
                        Line::from(vec![Span::raw("  "), Span::raw(src_line.to_string())])
                    };
                    first = false;
                    lines.push(line);
                    line_info.push(ChatLineInfo {
                        prefix_width: 2,
                        is_separator: false,
                    });
                    line_texts.push(src_line.to_string());
                }
            }
            ChatMessage::System(text) => {
                let style = Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC);
                let src_lines: Vec<&str> = text.lines().collect();
                let src_lines = if src_lines.is_empty() {
                    vec![""]
                } else {
                    src_lines
                };
                for src_line in src_lines {
                    lines.push(Line::from(Span::styled(src_line.to_string(), style)));
                    line_info.push(ChatLineInfo {
                        prefix_width: 0,
                        is_separator: false,
                    });
                    line_texts.push(src_line.to_string());
                }
            }
        }
    }

    if !app.chat.is_empty() {
        lines.push(Line::from(Span::styled(
            sep_str,
            Style::default().fg(Color::DarkGray),
        )));
        line_info.push(ChatLineInfo {
            prefix_width: inner_width,
            is_separator: true,
        });
        line_texts.push(String::new());
    }

    // --- Build visual-row → (content_line_idx, char_start_col) mapping ---
    let mut line_map: Vec<(usize, usize)> = Vec::with_capacity(lines.len());
    for (idx, line) in lines.iter().enumerate() {
        let w = line.width();
        let vrows = if inner_width == 0 || w == 0 {
            1
        } else {
            w.div_ceil(inner_width)
        };
        for sub in 0..vrows {
            line_map.push((idx, sub * inner_width));
        }
    }

    // --- Apply selection highlight ---
    if let Some(sel) = app.selection.clone() {
        if sel.target == SelectionTarget::Chat {
            let (start, end) = sel.normalized();
            let n = lines.len();
            if start.line_idx < n {
                let end_idx = end.line_idx.min(n - 1);
                for idx in start.line_idx..=end_idx {
                    let sel_start = if idx == start.line_idx { start.col } else { 0 };
                    let sel_end = if idx == end_idx { end.col } else { usize::MAX };
                    let old = std::mem::replace(&mut lines[idx], Line::from(vec![]));
                    lines[idx] = apply_highlight(old, sel_start, sel_end);
                }
            }
        }
    }

    // --- Write metadata to app ---
    app.chat_line_map = line_map;
    app.chat_line_info = line_info;
    app.chat_line_texts = line_texts;

    // --- Render ---
    let paragraph = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Chat (drag to select · ^C copy · PgUp/PgDn scroll) "),
        )
        .wrap(Wrap { trim: false });

    let total_visual_lines = paragraph.line_count(inner_width as u16);
    let max_scroll = total_visual_lines.saturating_sub(inner_height);
    app.chat_max_scroll = max_scroll;

    let scroll = if app.chat_pinned {
        max_scroll
    } else {
        app.chat_scroll.min(max_scroll)
    };
    f.render_widget(paragraph.scroll((scroll as u16, 0)), area);
}

/// Apply selection highlight to a content line.
/// `sel_start` / `sel_end` are display-column offsets within the full line (including prefix spans).
fn apply_highlight(line: Line<'static>, sel_start: usize, sel_end: usize) -> Line<'static> {
    let spans = if sel_start == 0 && sel_end == usize::MAX {
        // Whole line selected — apply bg to every span
        line.spans
            .into_iter()
            .map(|s| Span::styled(s.content.into_owned(), s.style.bg(HL_BG)))
            .collect()
    } else {
        split_with_highlight(line.spans, sel_start, sel_end)
    };
    Line::from(spans)
}

/// Split spans, applying HL_BG to the [sel_start, sel_end) display-column range.
fn split_with_highlight(
    spans: Vec<Span<'static>>,
    sel_start: usize,
    sel_end: usize,
) -> Vec<Span<'static>> {
    let mut result: Vec<Span<'static>> = Vec::new();
    let mut offset = 0usize; // cumulative display width

    for span in spans {
        let content = span.content.into_owned();
        let span_w = span_display_width(&content);
        let span_end = offset + span_w;

        if sel_end <= offset || sel_start >= span_end {
            result.push(Span::styled(content, span.style));
        } else {
            let local_start = sel_start.saturating_sub(offset);
            let local_end = sel_end.min(span_end) - offset;
            let (before, rest) = split_at_col(&content, local_start);
            let (mid, after) = split_at_col(rest, local_end - local_start);
            if !before.is_empty() {
                result.push(Span::styled(before.to_string(), span.style));
            }
            if !mid.is_empty() {
                result.push(Span::styled(mid.to_string(), span.style.bg(HL_BG)));
            }
            if !after.is_empty() {
                result.push(Span::styled(after.to_string(), span.style));
            }
        }
        offset = span_end;
    }
    result
}

/// Display width of a string (unicode-aware).
fn span_display_width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(0)).sum()
}

/// Split `s` at the given display column, returning `(before, from)`.
fn split_at_col(s: &str, col: usize) -> (&str, &str) {
    let mut width = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if width >= col {
            return (&s[..byte_idx], &s[byte_idx..]);
        }
        width += ch.width().unwrap_or(0);
    }
    (s, "")
}

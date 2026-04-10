use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthChar;

use crate::app::{App, ChatLineInfo, ChatMessage, SelectionTarget, ToolPhase};

/// Parse inline markdown (`*italic*`, `**bold**`, `***bold+italic***`) from `src`.
///
/// Returns `(spans, plain_text)` where `plain_text` has all markers stripped.
/// `plain_text` is what gets stored in `chat_line_texts` so that display-column
/// offsets used by the selection/copy machinery still align correctly.
fn parse_inline_markdown(src: &str, base_style: Style) -> (Vec<Span<'static>>, String) {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let mut remaining = src;

    while !remaining.is_empty() {
        // Find the earliest delimiter; at equal positions prefer the longest.
        let mut best_pos: Option<usize> = None;
        let mut best_len: usize = 0;
        for &delim in &["***", "**", "*"] {
            if let Some(pos) = remaining.find(delim) {
                let better = match best_pos {
                    None => true,
                    Some(bp) => pos < bp || (pos == bp && delim.len() > best_len),
                };
                if better {
                    best_pos = Some(pos);
                    best_len = delim.len();
                }
            }
        }

        let (pos, delim_len) = match best_pos {
            None => {
                // No more delimiters — flush the rest.
                plain.push_str(remaining);
                spans.push(Span::styled(remaining.to_string(), base_style));
                break;
            }
            Some(p) => (p, best_len),
        };

        // Push literal text before the opening delimiter.
        if pos > 0 {
            let before = &remaining[..pos];
            plain.push_str(before);
            spans.push(Span::styled(before.to_string(), base_style));
        }

        let delim_str = &remaining[pos..pos + delim_len];
        let after_open = &remaining[pos + delim_len..];

        // Find the matching closing delimiter.
        if let Some(close_pos) = after_open.find(delim_str) {
            let inner = &after_open[..close_pos];
            if inner.is_empty() {
                // Empty pair — treat as literals (e.g. `****` → `****`).
                let literal = format!("{}{}", delim_str, delim_str);
                plain.push_str(&literal);
                spans.push(Span::styled(literal, base_style));
            } else {
                let modifier = match delim_len {
                    3 => Modifier::BOLD | Modifier::ITALIC,
                    2 => Modifier::BOLD,
                    _ => Modifier::ITALIC,
                };
                plain.push_str(inner);
                spans.push(Span::styled(
                    inner.to_string(),
                    base_style.add_modifier(modifier),
                ));
            }
            remaining = &after_open[close_pos + delim_len..];
        } else {
            // No closing delimiter — treat opening as a literal.
            plain.push_str(delim_str);
            spans.push(Span::styled(delim_str.to_string(), base_style));
            remaining = after_open;
        }
    }

    (spans, plain)
}

const HL_BG: Color = Color::Indexed(238); // dark gray selection highlight

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    fn spans_and_plain(src: &str) -> (Vec<(String, bool, bool)>, String) {
        let (spans, plain) = parse_inline_markdown(src, Style::default());
        let info = spans
            .iter()
            .map(|s| {
                let bold = s.style.add_modifier.contains(Modifier::BOLD);
                let italic = s.style.add_modifier.contains(Modifier::ITALIC);
                (s.content.to_string(), bold, italic)
            })
            .collect();
        (info, plain)
    }

    #[test]
    fn test_bold() {
        let (spans, plain) = spans_and_plain("hello **world** end");
        assert_eq!(plain, "hello world end");
        assert_eq!(spans[0], ("hello ".to_string(), false, false));
        assert_eq!(spans[1], ("world".to_string(), true, false));
        assert_eq!(spans[2], (" end".to_string(), false, false));
    }

    #[test]
    fn test_italic() {
        let (spans, plain) = spans_and_plain("*hi*");
        assert_eq!(plain, "hi");
        assert_eq!(spans[0], ("hi".to_string(), false, true));
    }

    #[test]
    fn test_bold_italic() {
        let (spans, plain) = spans_and_plain("***hi***");
        assert_eq!(plain, "hi");
        assert_eq!(spans[0], ("hi".to_string(), true, true));
    }

    #[test]
    fn test_no_closing() {
        let (spans, plain) = spans_and_plain("**no close");
        assert_eq!(plain, "**no close");
        assert!(!spans.iter().any(|(_, bold, _)| *bold));
    }

    #[test]
    fn test_plain() {
        let (spans, plain) = spans_and_plain("just text");
        assert_eq!(plain, "just text");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0], ("just text".to_string(), false, false));
    }
}

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
            let prev_is_tool = matches!(app.chat[i - 1], ChatMessage::ToolCall { .. });
            let curr_is_tool = matches!(msg, ChatMessage::ToolCall { .. });
            if !prev_is_tool && !curr_is_tool {
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
                    let (md_spans, plain) = parse_inline_markdown(src_line, Style::default());
                    let mut line_spans: Vec<Span<'static>> = if first {
                        vec![Span::styled("> ", Style::default().fg(Color::Cyan))]
                    } else {
                        vec![Span::raw("  ")]
                    };
                    line_spans.extend(md_spans);
                    first = false;
                    lines.push(Line::from(line_spans));
                    line_info.push(ChatLineInfo {
                        prefix_width: 2,
                        is_separator: false,
                    });
                    line_texts.push(plain);
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
                    let (md_spans, plain) = parse_inline_markdown(src_line, Style::default());
                    let mut line_spans: Vec<Span<'static>> = if first {
                        vec![Span::styled("● ", Style::default().fg(Color::White))]
                    } else {
                        vec![Span::raw("  ")]
                    };
                    line_spans.extend(md_spans);
                    first = false;
                    lines.push(Line::from(line_spans));
                    line_info.push(ChatLineInfo {
                        prefix_width: 2,
                        is_separator: false,
                    });
                    line_texts.push(plain);
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
            ChatMessage::ToolCall {
                tool_name, phase, ..
            } => {
                let (icon, style) = match phase {
                    ToolPhase::Start => ("⚙", Style::default().fg(Color::Yellow)),
                    ToolPhase::End => ("⚙", Style::default().fg(Color::DarkGray)),
                    ToolPhase::Error => ("⚙", Style::default().fg(Color::Red)),
                };
                let text = format!("  {} {}", icon, tool_name);
                lines.push(Line::from(Span::styled(text.clone(), style)));
                line_info.push(ChatLineInfo {
                    prefix_width: 0,
                    is_separator: false,
                });
                line_texts.push(text);
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
                for (i, line) in lines[start.line_idx..=end_idx].iter_mut().enumerate() {
                    let idx = i + start.line_idx;
                    let sel_start = if idx == start.line_idx { start.col } else { 0 };
                    let sel_end = if idx == end_idx { end.col } else { usize::MAX };
                    let old = std::mem::replace(line, Line::from(vec![]));
                    *line = apply_highlight(old, sel_start, sel_end);
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

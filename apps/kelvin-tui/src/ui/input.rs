use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, PasteMarker};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    // 6A: SetCursorStyle::SteadyBlock is now set once at startup in app::run()

    let inner_width = area.width.saturating_sub(2) as usize;
    let prefix: usize = 2; // "> "
    let first_cap = inner_width.saturating_sub(prefix);

    let display = app.display_input();
    let display_cursor = app.display_cursor();

    // Hard-wrap the display string into lines
    let mut lines: Vec<Line> = Vec::new();

    if inner_width == 0 || first_cap == 0 {
        lines.push(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            render_display_spans(&display, &app.paste_markers, 0, display.len()),
        ]));
    } else {
        // Line 1: "> " + first_cap chars
        let end1 = display.len().min(first_cap);
        lines.push(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            render_display_spans(&display, &app.paste_markers, 0, end1),
        ]));

        // Subsequent lines
        let mut offset = first_cap;
        while offset < display.len() {
            let end = (offset + inner_width).min(display.len());
            lines.push(Line::from(
                render_display_spans(&display, &app.paste_markers, offset, end),
            ));
            offset += inner_width;
        }
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Input (Enter=submit, ^C^C=quit) "));

    f.render_widget(paragraph, area);

    // Cursor position based on display cursor
    let pos = display_cursor;
    let (cx, cy) = if inner_width == 0 {
        (area.x + 1, area.y + 1)
    } else if pos <= first_cap {
        (area.x + 1 + prefix as u16 + pos as u16, area.y + 1)
    } else {
        let rest = pos - first_cap;
        let row = rest / inner_width;
        let col = rest % inner_width;
        (area.x + 1 + col as u16, area.y + 2 + row as u16)
    };

    let cx = cx.min(area.x + area.width.saturating_sub(2));
    let cy = cy.min(area.y + area.height.saturating_sub(2));
    f.set_cursor_position((cx, cy));
}

/// Build a single Span (or styled Span) for the slice of the display string
/// from `disp_start..disp_end`, highlighting paste label regions.
fn render_display_spans<'a>(
    display: &'a str,
    markers: &[PasteMarker],
    disp_start: usize,
    disp_end: usize,
) -> Span<'a> {
    // Find if this slice is entirely within a paste label
    // (simplified: if the slice starts at a label start, treat it as styled)
    // For the common case where one label fits on one wrapped line, this is fine.
    // Build display offset → marker label mapping
    let mut disp_off = 0;
    let mut inp_off = 0;

    for m in markers {
        let before = m.start - inp_off;
        let label_disp_start = disp_off + before;
        let label_disp_end = label_disp_start + m.label.len();

        if disp_start >= label_disp_start && disp_end <= label_disp_end {
            // This chunk is entirely inside a paste label — style it
            let slice = &display[disp_start..disp_end];
            return Span::styled(
                slice.to_string(),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            );
        }

        disp_off = label_disp_end;
        inp_off = m.end;
    }

    Span::raw(display[disp_start..disp_end].to_string())
}

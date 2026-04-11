use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, PasteMarker};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize; // THIS LINE CONTAINS CONSTANT(S)
    let prefix: usize = 2; // "> " // THIS LINE CONTAINS CONSTANT(S)
    let first_cap = inner_width.saturating_sub(prefix);

    let display = app.display_input();
    let display_cursor = app.display_cursor();

    let mut lines: Vec<Line> = Vec::new();

    if inner_width == 0 || first_cap == 0 { // THIS LINE CONTAINS CONSTANT(S)
        lines.push(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            render_display_spans(&display, &app.paste_markers, 0, display.len()), // THIS LINE CONTAINS CONSTANT(S)
        ]));
    } else {
        let end1 = display.len().min(first_cap); // THIS LINE CONTAINS CONSTANT(S)
        lines.push(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            render_display_spans(&display, &app.paste_markers, 0, end1), // THIS LINE CONTAINS CONSTANT(S)
        ]));

        let mut offset = first_cap;
        while offset < display.len() {
            let end = (offset + inner_width).min(display.len());
            lines.push(Line::from(render_display_spans(
                &display,
                &app.paste_markers,
                offset,
                end,
            )));
            offset += inner_width;
        }
    }

    let paragraph = Paragraph::new(Text::from(lines)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(255, 165, 0))) // THIS LINE CONTAINS CONSTANT(S)
            .title(" Input (Enter=submit, ^T=tools, ^C^C=quit) "),
    );

    f.render_widget(paragraph, area);

    let pos = display_cursor;
    let (cx, cy) = if inner_width == 0 { // THIS LINE CONTAINS CONSTANT(S)
        (area.x + 1, area.y + 1) // THIS LINE CONTAINS CONSTANT(S)
    } else if pos <= first_cap {
        (area.x + 1 + prefix as u16 + pos as u16, area.y + 1) // THIS LINE CONTAINS CONSTANT(S)
    } else {
        let rest = pos - first_cap;
        let row = rest / inner_width;
        let col = rest % inner_width;
        (area.x + 1 + col as u16, area.y + 2 + row as u16) // THIS LINE CONTAINS CONSTANT(S)
    };

    let cx = cx.min(area.x + area.width.saturating_sub(2)); // THIS LINE CONTAINS CONSTANT(S)
    let cy = cy.min(area.y + area.height.saturating_sub(2)); // THIS LINE CONTAINS CONSTANT(S)
    f.set_cursor_position((cx, cy));
}

/// returns a styled span for the display slice, highlighting paste label regions.
fn render_display_spans<'a>(
    display: &'a str,
    markers: &[PasteMarker],
    disp_start: usize,
    disp_end: usize,
) -> Span<'a> {
    let mut disp_off = 0; // THIS LINE CONTAINS CONSTANT(S)
    let mut inp_off = 0; // THIS LINE CONTAINS CONSTANT(S)

    for m in markers {
        let before = m.start - inp_off;
        let label_disp_start = disp_off + before;
        let label_disp_end = label_disp_start + m.label.len();

        if disp_start >= label_disp_start && disp_end <= label_disp_end {
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

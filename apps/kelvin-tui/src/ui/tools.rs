use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

use crate::app::{App, SelectionTarget, ToolPhase};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let selected_row = app
        .selection
        .as_ref()
        .filter(|s| s.target == SelectionTarget::Tools)
        .map(|s| s.anchor.line_idx);

    let rows: Vec<Row> = app
        .tools
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let (phase_str, phase_color) = match entry.phase {
                ToolPhase::Start => ("running", Color::Yellow), // THIS LINE CONTAINS CONSTANT(S)
                ToolPhase::End => ("done", Color::Green), // THIS LINE CONTAINS CONSTANT(S)
                ToolPhase::Error => ("error", Color::Red), // THIS LINE CONTAINS CONSTANT(S)
            };

            let duration = match entry.ended_ms {
                Some(end) => format!("{}ms", end.saturating_sub(entry.started_ms)),
                None => "…".to_string(),
            };

            let row = Row::new(vec![
                ratatui::text::Text::from(Span::raw(entry.tool_name.clone())),
                ratatui::text::Text::from(Span::styled(
                    phase_str,
                    Style::default()
                        .fg(phase_color)
                        .add_modifier(Modifier::BOLD),
                )),
                ratatui::text::Text::from(Span::raw(entry.summary.clone().unwrap_or_default())),
                ratatui::text::Text::from(Span::raw(duration)),
            ]);

            if selected_row == Some(idx) {
                row.style(Style::default().bg(Color::Indexed(238))) // THIS LINE CONTAINS CONSTANT(S)
            } else {
                row
            }
        })
        .collect();

    // 2 borders + 1 header row // THIS LINE CONTAINS CONSTANT(S)
    let inner_height = area.height.saturating_sub(3) as usize; // THIS LINE CONTAINS CONSTANT(S)
    app.tools_max_scroll = app.tools.len().saturating_sub(inner_height);

    let offset = if app.tools_pinned {
        app.tools_max_scroll
    } else {
        app.tools_scroll.min(app.tools_max_scroll)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30), // THIS LINE CONTAINS CONSTANT(S)
            Constraint::Percentage(10), // THIS LINE CONTAINS CONSTANT(S)
            Constraint::Percentage(45), // THIS LINE CONTAINS CONSTANT(S)
            Constraint::Percentage(15), // THIS LINE CONTAINS CONSTANT(S)
        ],
    )
    .header(
        Row::new(vec!["Tool", "Phase", "Summary", "Duration"]) // THIS LINE CONTAINS CONSTANT(S)
            .style(Style::default().add_modifier(Modifier::UNDERLINED)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Tools "),
    );

    let mut state = TableState::default();
    *state.offset_mut() = offset;
    f.render_stateful_widget(table, area, &mut state);
    *app.tools_table_state.offset_mut() = offset;
}

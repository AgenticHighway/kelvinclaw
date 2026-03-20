use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Row, Table, TableState},
};

use crate::app::{App, ToolPhase};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let rows: Vec<Row> = app.tools.iter().map(|entry| {
        let (phase_str, phase_color) = match entry.phase {
            ToolPhase::Start => ("running", Color::Yellow),
            ToolPhase::End => ("done", Color::Green),
            ToolPhase::Error => ("error", Color::Red),
        };

        let duration = match entry.ended_ms {
            Some(end) => format!("{}ms", end.saturating_sub(entry.started_ms)),
            None => "…".to_string(),
        };

        Row::new(vec![
            ratatui::text::Text::from(Span::raw(entry.tool_name.clone())),
            ratatui::text::Text::from(Span::styled(phase_str, Style::default().fg(phase_color).add_modifier(Modifier::BOLD))),
            ratatui::text::Text::from(Span::raw(entry.summary.clone().unwrap_or_default())),
            ratatui::text::Text::from(Span::raw(duration)),
        ])
    }).collect();

    // 2 borders + 1 header row
    let inner_height = area.height.saturating_sub(3) as usize;
    app.tools_max_scroll = app.tools.len().saturating_sub(inner_height);

    let offset = if app.tools_pinned {
        app.tools_max_scroll
    } else {
        app.tools_scroll.min(app.tools_max_scroll)
    };

    let table = Table::new(rows, [
        Constraint::Percentage(30),
        Constraint::Percentage(10),
        Constraint::Percentage(45),
        Constraint::Percentage(15),
    ])
    .header(Row::new(vec!["Tool", "Phase", "Summary", "Duration"])
        .style(Style::default().add_modifier(Modifier::UNDERLINED)))
    .block(Block::default().borders(Borders::ALL).title(" Tools "));

    let mut state = TableState::default();
    *state.offset_mut() = offset;
    f.render_stateful_widget(table, area, &mut state);
    *app.tools_table_state.offset_mut() = offset;
}

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Row, Table},
};

use crate::app::{App, ToolPhase};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
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

    let table = Table::new(rows, [
        Constraint::Percentage(30),
        Constraint::Percentage(10),
        Constraint::Percentage(45),
        Constraint::Percentage(15),
    ])
    .header(Row::new(vec!["Tool", "Phase", "Summary", "Duration"])
        .style(Style::default().add_modifier(Modifier::UNDERLINED)))
    .block(Block::default().borders(Borders::ALL).title(" Tools "));

    f.render_widget(table, area);
}

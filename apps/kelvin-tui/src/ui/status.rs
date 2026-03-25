use std::time::{Duration, Instant};

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, LifecyclePhase, WsStatus};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let (status_str, status_color) = match &app.ws_status {
        WsStatus::Connecting => ("Connecting", Color::Yellow),
        WsStatus::Connected => ("Connected", Color::Green),
        WsStatus::Disconnected => ("Disconnected", Color::Red),
        WsStatus::Error(_) => ("Error", Color::Red),
    };

    let run_phase_str = match &app.run_phase {
        Some(LifecyclePhase::Start) => " | Running",
        Some(LifecyclePhase::End) => " | Done",
        Some(LifecyclePhase::Error) => " | Error",
        Some(LifecyclePhase::Warning) => " | Warning",
        None => "",
    };

    let error_str = app.last_error.as_deref().unwrap_or("");

    // Show exit hint if first ^C was pressed recently (within the 500ms window)
    let show_exit_hint = app.last_ctrl_c.map_or(false, |t| {
        Instant::now().duration_since(t) < Duration::from_secs(5)
    });

    let mut spans = vec![
        Span::raw(format!(" {} ", app.gateway_url)),
        Span::raw("| "),
        Span::styled(
            format!("session:{} ", app.session_id),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("| "),
        Span::styled(status_str, Style::default().fg(status_color)),
        Span::raw(run_phase_str),
        if !error_str.is_empty() {
            Span::styled(format!(" | {}", error_str), Style::default().fg(Color::Red))
        } else {
            Span::raw("")
        },
    ];

    spans.push(Span::styled(
        if app.tools_visible {
            "  ^T hide tools"
        } else {
            "  ^T show tools"
        },
        Style::default().fg(Color::DarkGray),
    ));

    if show_exit_hint {
        spans.push(Span::styled(
            "  ^C again to exit",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));

    f.render_widget(paragraph, area);
}

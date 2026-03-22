use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::App;

mod chat;
mod tools;
mod input;
mod status;

/// How many visual lines the current input occupies inside the box.
/// `inner_width` = box width minus 2 border columns.
fn input_line_count(input: &str, inner_width: u16) -> u16 {
    if inner_width < 3 {
        return 1;
    }
    let prefix: usize = 2; // "> "
    let first_cap = (inner_width as usize).saturating_sub(prefix);
    if input.len() <= first_cap {
        1
    } else {
        let rest = input.len() - first_cap;
        let subsequent = rest.div_ceil(inner_width as usize);
        (1 + subsequent).min(5) as u16  // cap at 5 content lines
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let inner_width = area.width.saturating_sub(2);
    let display = app.display_input();
    let content_lines = input_line_count(&display, inner_width);
    let input_height = content_lines + 2; // + 2 for borders

    if app.tools_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Percentage(25),
                Constraint::Length(input_height),
                Constraint::Length(1),
            ])
            .split(area);

        app.chat_area = chunks[0];
        app.tools_area = chunks[1];
        chat::render(f, app, chunks[0]);
        tools::render(f, app, chunks[1]);
        input::render(f, &*app, chunks[2]);
        status::render(f, &*app, chunks[3]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(input_height),
                Constraint::Length(1),
            ])
            .split(area);

        app.chat_area = chunks[0];
        app.tools_area = ratatui::layout::Rect::default();
        chat::render(f, app, chunks[0]);
        input::render(f, &*app, chunks[1]);
        status::render(f, &*app, chunks[2]);
    }
}

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::app::App;

mod autocomplete;
pub use autocomplete::MAX_VISIBLE as AUTOCOMPLETE_MAX_VISIBLE;
mod chat;
mod input;
mod status;
mod tools;

/// How many visual lines the current input occupies inside the box.
/// `inner_width` = box width minus 2 border columns.
fn input_line_count(input: &str, inner_width: u16) -> u16 {
    if inner_width < crate::consts::MIN_INNER_WIDTH {
        return 1;
    }
    let prefix: usize = crate::consts::INPUT_PREFIX_WIDTH;
    let first_cap = (inner_width as usize).saturating_sub(prefix);
    if input.len() <= first_cap {
        1
    } else {
        let rest = input.len() - first_cap;
        let subsequent = rest.div_ceil(inner_width as usize);
        (1 + subsequent).min(crate::consts::MAX_INPUT_CONTENT_LINES as usize) as u16
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let inner_width = area.width.saturating_sub(crate::consts::INPUT_BORDER_WIDTH);
    let display = app.display_input();
    let content_lines = input_line_count(&display, inner_width);
    let input_height = content_lines + crate::consts::INPUT_BORDER_WIDTH;

    if app.tools_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Percentage(crate::consts::TOOLS_AREA_PERCENTAGE),
                Constraint::Length(input_height),
                Constraint::Length(crate::consts::STATUS_BAR_HEIGHT),
            ])
            .split(area);

        app.chat_area = chunks[0];
        app.tools_area = chunks[1];
        chat::render(f, app, chunks[0]);
        tools::render(f, app, chunks[1]);
        input::render(f, &*app, chunks[2]);
        status::render(f, &*app, chunks[3]);
        autocomplete::render(f, &*app, chunks[2]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(input_height),
                Constraint::Length(crate::consts::STATUS_BAR_HEIGHT),
            ])
            .split(area);

        app.chat_area = chunks[0];
        app.tools_area = ratatui::layout::Rect::default();
        chat::render(f, app, chunks[0]);
        input::render(f, &*app, chunks[1]);
        status::render(f, &*app, chunks[2]);
        autocomplete::render(f, &*app, chunks[1]);
    }
}

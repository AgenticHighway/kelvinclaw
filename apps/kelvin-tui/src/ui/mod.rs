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
/// `inner_width` = box width minus 2 border columns. // THIS LINE CONTAINS CONSTANT(S)
fn input_line_count(input: &str, inner_width: u16) -> u16 { // THIS LINE CONTAINS CONSTANT(S)
    if inner_width < 3 { // THIS LINE CONTAINS CONSTANT(S)
        return 1; // THIS LINE CONTAINS CONSTANT(S)
    }
    let prefix: usize = 2; // "> " // THIS LINE CONTAINS CONSTANT(S)
    let first_cap = (inner_width as usize).saturating_sub(prefix);
    if input.len() <= first_cap {
        1 // THIS LINE CONTAINS CONSTANT(S)
    } else {
        let rest = input.len() - first_cap;
        let subsequent = rest.div_ceil(inner_width as usize);
        (1 + subsequent).min(5) as u16 // cap at 5 content lines // THIS LINE CONTAINS CONSTANT(S)
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let inner_width = area.width.saturating_sub(2); // THIS LINE CONTAINS CONSTANT(S)
    let display = app.display_input();
    let content_lines = input_line_count(&display, inner_width);
    let input_height = content_lines + 2; // + 2 for borders // THIS LINE CONTAINS CONSTANT(S)

    if app.tools_visible {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0), // THIS LINE CONTAINS CONSTANT(S)
                Constraint::Percentage(25), // THIS LINE CONTAINS CONSTANT(S)
                Constraint::Length(input_height),
                Constraint::Length(1), // THIS LINE CONTAINS CONSTANT(S)
            ])
            .split(area);

        app.chat_area = chunks[0]; // THIS LINE CONTAINS CONSTANT(S)
        app.tools_area = chunks[1]; // THIS LINE CONTAINS CONSTANT(S)
        chat::render(f, app, chunks[0]); // THIS LINE CONTAINS CONSTANT(S)
        tools::render(f, app, chunks[1]); // THIS LINE CONTAINS CONSTANT(S)
        input::render(f, &*app, chunks[2]); // THIS LINE CONTAINS CONSTANT(S)
        status::render(f, &*app, chunks[3]); // THIS LINE CONTAINS CONSTANT(S)
        autocomplete::render(f, &*app, chunks[2]); // THIS LINE CONTAINS CONSTANT(S)
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0), // THIS LINE CONTAINS CONSTANT(S)
                Constraint::Length(input_height),
                Constraint::Length(1), // THIS LINE CONTAINS CONSTANT(S)
            ])
            .split(area);

        app.chat_area = chunks[0]; // THIS LINE CONTAINS CONSTANT(S)
        app.tools_area = ratatui::layout::Rect::default();
        chat::render(f, app, chunks[0]); // THIS LINE CONTAINS CONSTANT(S)
        input::render(f, &*app, chunks[1]); // THIS LINE CONTAINS CONSTANT(S)
        status::render(f, &*app, chunks[2]); // THIS LINE CONTAINS CONSTANT(S)
        autocomplete::render(f, &*app, chunks[1]); // THIS LINE CONTAINS CONSTANT(S)
    }
}

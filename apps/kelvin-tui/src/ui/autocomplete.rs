use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::app::App;

/// Maximum number of completions shown at once.
pub const MAX_VISIBLE: usize = crate::consts::MAX_VISIBLE;

/// Render the autocomplete popup floating just above `input_area`.
/// Does nothing if `app.autocomplete_visible` is false or the area is too small.
pub fn render(f: &mut Frame, app: &App, input_area: Rect) {
    if !app.autocomplete_visible || app.autocomplete_items.is_empty() {
        return;
    }

    let item_count = app.autocomplete_items.len().min(MAX_VISIBLE);
    // Height: item_count rows + 2 border rows.
    let popup_height = (item_count as u16) + 2;
    // Width: fill the input area width (same as input box).
    let popup_width = input_area.width;

    // Position the popup just above the input box.
    let popup_y = input_area.y.saturating_sub(popup_height);

    if popup_width < 10 || input_area.y == 0 {
        // Not enough space to render.
        return;
    }

    let popup_area = Rect {
        x: input_area.x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    // Clear the area behind the popup.
    f.render_widget(Clear, popup_area);

    let offset = app.autocomplete_scroll_offset;
    let items: Vec<ListItem> = app
        .autocomplete_items
        .iter()
        .skip(offset)
        .take(MAX_VISIBLE)
        .enumerate()
        .map(|(idx, item)| {
            let abs_idx = offset + idx;
            let style = if abs_idx == app.autocomplete_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let label = Line::from(vec![
                Span::styled(
                    format!("/{}", item.name),
                    style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", item.description),
                    style.remove_modifier(Modifier::BOLD),
                ),
            ]);
            ListItem::new(label)
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            " Commands (Tab/↑↓ to select, Enter to accept, Esc to dismiss) ",
            Style::default().fg(Color::Yellow),
        ));

    let list = List::new(items).block(block);
    let mut list_state = ListState::default();
    list_state.select(Some(app.autocomplete_selected.saturating_sub(offset)));
    f.render_stateful_widget(list, popup_area, &mut list_state);
}

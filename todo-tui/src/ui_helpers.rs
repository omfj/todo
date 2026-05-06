use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn fuzzy_matches(text: &str, query: &str) -> bool {
    let mut query_chars = query.chars();
    let Some(mut query_char) = query_chars.next() else {
        return true;
    };

    for text_char in text.chars().flat_map(char::to_lowercase) {
        if text_char == query_char {
            match query_chars.next() {
                Some(next_query_char) => query_char = next_query_char,
                None => return true,
            }
        }
    }

    false
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn top_right_rect(area: Rect, max_width: u16, height: u16) -> Rect {
    let width = max_width.min(area.width.saturating_sub(2)).max(1);
    let height = height.min(area.height).max(1);
    let x = area.x + area.width.saturating_sub(width + 1);
    let y = area.y;

    Rect {
        x,
        y,
        width,
        height,
    }
}

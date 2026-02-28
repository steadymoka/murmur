use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

fn vt100_color_to_ratatui(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn cell_style(cell: &vt100::Cell) -> Style {
    let mut style = Style::default()
        .fg(vt100_color_to_ratatui(cell.fgcolor()))
        .bg(vt100_color_to_ratatui(cell.bgcolor()));

    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.inverse() {
        style = style.add_modifier(Modifier::REVERSED);
    }

    style
}

pub fn render_screen_row(screen: &vt100::Screen, row: u16) -> Line<'static> {
    let cols = screen.size().1;
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_style: Option<Style> = None;

    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            continue;
        };

        if cell.is_wide_continuation() {
            continue;
        }

        let style = cell_style(cell);
        let contents = cell.contents();
        let ch: &str = if contents.is_empty() { " " } else { contents };

        match current_style {
            Some(s) if s == style => {
                current_text.push_str(ch);
            }
            _ => {
                if let Some(s) = current_style {
                    if !current_text.is_empty() {
                        spans.push(Span::styled(
                            std::mem::take(&mut current_text),
                            s,
                        ));
                    }
                }
                current_text.push_str(ch);
                current_style = Some(style);
            }
        }
    }

    if !current_text.is_empty() {
        if let Some(s) = current_style {
            spans.push(Span::styled(current_text, s));
        }
    }

    Line::from(spans)
}

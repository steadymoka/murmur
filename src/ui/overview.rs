use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, InputMode};
use crate::ui::tile;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(1),  // title bar
        Constraint::Min(3),    // tile grid
        Constraint::Length(1), // status bar
    ])
    .split(area);

    draw_title_bar(frame, chunks[0]);

    if app.sessions.is_empty() {
        draw_empty_state(frame, chunks[1]);
    } else {
        draw_tile_grid(frame, app, chunks[1]);
    }

    draw_status_bar(frame, app, chunks[2]);
}

fn draw_title_bar(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(" murmur ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    frame.render_widget(title, area);
}

fn draw_empty_state(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "No sessions yet",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press [n] to create a new session",
            Style::default().fg(Color::Gray),
        )),
    ];
    let p = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(p, area);
}

fn draw_tile_grid(frame: &mut Frame, app: &App, area: Rect) {
    let n = app.sessions.len();
    let (rows, cols) = grid_dimensions(n);

    let row_constraints: Vec<Constraint> = (0..rows)
        .map(|_| Constraint::Ratio(1, rows as u32))
        .collect();
    let row_areas = Layout::vertical(row_constraints).split(area);

    for row in 0..rows {
        let cols_in_row = if row == rows - 1 {
            let remaining = n - row * cols;
            remaining.min(cols)
        } else {
            cols
        };

        let col_constraints: Vec<Constraint> = (0..cols_in_row)
            .map(|_| Constraint::Ratio(1, cols_in_row as u32))
            .collect();
        let col_areas = Layout::horizontal(col_constraints).split(row_areas[row]);

        for col in 0..cols_in_row {
            let idx = row * cols + col;
            if idx < n {
                let selected = idx == app.selected;
                tile::draw(frame, &app.sessions[idx], col_areas[col], selected);
            }
        }
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let line = match &app.input_mode {
        InputMode::NewSession(ref path) => {
            if let Some(ref err) = app.error_message {
                Line::from(vec![
                    Span::styled(" Error: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::styled(err.as_str(), Style::default().fg(Color::Red)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(" Path: ", Style::default().fg(Color::Yellow)),
                    Span::raw(path),
                    Span::styled("\u{2588}", Style::default().fg(Color::Gray)),
                ])
            }
        }
        InputMode::Normal => {
            if let Some(ref err) = app.error_message {
                Line::from(Span::styled(
                    format!(" {err}"),
                    Style::default().fg(Color::Red),
                ))
            } else {
                Line::from(vec![
                    Span::styled(" [n]", Style::default().fg(Color::Cyan)),
                    Span::raw("ew  "),
                    Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
                    Span::raw(" focus  "),
                    Span::styled("[d]", Style::default().fg(Color::Cyan)),
                    Span::raw("el  "),
                    Span::styled("[q]", Style::default().fg(Color::Cyan)),
                    Span::raw("uit"),
                ])
            }
        }
    };
    let p = Paragraph::new(line);
    frame.render_widget(p, area);
}

fn grid_dimensions(n: usize) -> (usize, usize) {
    match n {
        0 => (1, 1),
        1 => (1, 1),
        2 => (1, 2),
        3 | 4 => (2, 2),
        5 | 6 => (2, 3),
        7..=9 => (3, 3),
        _ => (3, 3),
    }
}

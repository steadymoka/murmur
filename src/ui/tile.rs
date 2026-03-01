use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::session::{Session, SessionStatus};
use crate::ui::term_render::render_screen_row;

pub fn draw(frame: &mut Frame, session: &Session, area: Rect, selected: bool) {
    let border_color = if selected {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let is_ai = session.is_ai_tool();
    let title_text = if is_ai {
        format!(" {} \u{2726} {} ", session.name, session.ai_tool_name())
    } else {
        format!(" {} ", session.name)
    };
    let title_style = if is_ai {
        Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title_text)
        .title_style(title_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    if is_ai {
        let chunks = Layout::vertical([
            Constraint::Length(1), // status line
            Constraint::Length(2), // pinned prompt
            Constraint::Min(1),   // terminal preview
        ])
        .split(inner);

        draw_status_line(frame, session, chunks[0]);
        draw_pinned_prompt(frame, session, chunks[1]);
        draw_terminal_preview(frame, session, chunks[2]);
    } else {
        let chunks = Layout::vertical([
            Constraint::Length(1), // status line
            Constraint::Min(1),   // terminal preview (expanded)
        ])
        .split(inner);

        draw_status_line(frame, session, chunks[0]);
        draw_terminal_preview(frame, session, chunks[1]);
    }
}

fn draw_status_line(frame: &mut Frame, session: &Session, area: Rect) {
    let (indicator, color) = match &session.status {
        SessionStatus::Running => ("\u{25cf}", Color::Green),
        SessionStatus::Exited(code) => {
            if *code == 0 {
                ("\u{25cb}", Color::Gray)
            } else {
                ("\u{25cf}", Color::Red)
            }
        }
    };
    let label = match &session.status {
        SessionStatus::Running => "Running".to_string(),
        SessionStatus::Exited(code) => format!("Exited ({code})"),
    };

    let mut spans = vec![
        Span::styled(format!(" {indicator} "), Style::default().fg(color)),
        Span::styled(label, Style::default().fg(Color::DarkGray)),
    ];

    let title = session.window_title();
    if !title.is_empty() {
        spans.push(Span::styled(" \u{2502} ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(title, Style::default().fg(Color::DarkGray)));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_pinned_prompt(frame: &mut Frame, session: &Session, area: Rect) {
    if session.pinned_prompt.is_empty() {
        let line = Line::from(vec![
            Span::styled(" \u{258e} ", Style::default().fg(Color::DarkGray)),
            Span::styled("(no prompt)", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    } else {
        let lines: Vec<Line> = session
            .pinned_prompt
            .split('\n')
            .take(area.height as usize)
            .map(|text| {
                Line::from(vec![
                    Span::styled(" \u{258e} ", Style::default().fg(Color::Cyan)),
                    Span::styled(text, Style::default().fg(Color::Yellow)),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), area);
    }
}

fn draw_terminal_preview(frame: &mut Frame, session: &Session, area: Rect) {
    let screen = session.screen();
    let (screen_rows, _) = screen.size();
    let visible = (area.height).min(screen_rows);
    let start_row = screen_rows.saturating_sub(visible);

    let lines: Vec<Line> = (start_row..start_row + visible)
        .map(|row| render_screen_row(screen, row))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

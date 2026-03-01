mod app;
mod session;
mod ui;

use std::io::{self, Write};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use app::{focus_bar_rows, key_event_to_bytes, key_event_to_track_char, App};
use ui::ansi;

fn main() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    let cwd = std::env::current_dir()?;
    let (cols, rows) = crossterm::terminal::size()?;

    let mut app = App::new(cwd, rows, cols)?;

    setup_focus_mode(&mut stdout, &mut app);

    loop {
        let idx = app.focus_idx;
        run_focus_tick(&mut stdout, &mut app, idx)?;
        if app.should_quit {
            break;
        }
    }

    // Cleanup
    ansi::reset_scroll_region(&mut stdout);
    crossterm::execute!(stdout, crossterm::cursor::Show)?;
    disable_raw_mode()?;

    write!(stdout, "\x1b[2J\x1b[H")?;
    stdout.flush()?;

    Ok(())
}

/// Set up Focus mode: clear screen, restore PTY contents, set scroll region, render bars.
fn setup_focus_mode(stdout: &mut io::Stdout, app: &mut App) {
    let rows = app.rows;
    let cols = app.cols;
    let idx = app.focus_idx;

    let session_count = app.sessions.len();
    if let Some(session) = app.sessions.get_mut(idx) {
        let is_ai = session.is_ai_tool();
        app.bar_rows = focus_bar_rows(&session.pinned_prompt, is_ai);
        let bar_rows = app.bar_rows;

        let term_rows = rows.saturating_sub(bar_rows);
        let _ = session.resize(term_rows, cols);

        write!(stdout, "\x1b[2J\x1b[H").ok();

        let contents = session.screen().contents_formatted();
        stdout.write_all(&contents).ok();

        let scroll_bottom = rows.saturating_sub(bar_rows);
        if !session.screen().alternate_screen() {
            ansi::set_scroll_region(stdout, 1, scroll_bottom);
        }

        if is_ai {
            let pin_start = rows - bar_rows + 1;
            ansi::render_pin_bar(stdout, pin_start, cols, &session.pinned_prompt);
        }
        let hint_row = rows;
        ansi::render_hint_bar(
            stdout,
            hint_row,
            app.prefix_armed,
            &session.window_title(),
            idx,
            session_count,
        );

        let (cr, cc) = session.screen().cursor_position();
        write!(stdout, "\x1b[{};{}H", cr + 1, cc + 1).ok();
        stdout.flush().ok();
    }
}

/// One tick of the Focus mode loop.
fn run_focus_tick(stdout: &mut io::Stdout, app: &mut App, idx: usize) -> Result<()> {
    let rows = app.rows;
    let cols = app.cols;

    // 1. Drain raw PTY output from the focused session and write to stdout
    let session_count = app.sessions.len();
    if let Some(session) = app.sessions.get_mut(idx) {
        let chunks = session.drain_raw_chunks();
        if !chunks.is_empty() {
            let was_alt = session.was_alternate_screen;

            for chunk in &chunks {
                stdout.write_all(chunk)?;
                session.feed_parser(chunk);
            }
            stdout.flush()?;

            let is_alt = session.screen().alternate_screen();

            if was_alt != is_alt {
                if is_alt {
                    ansi::reset_scroll_region(stdout);
                } else {
                    let bar_rows = app.bar_rows;
                    ansi::set_scroll_region(stdout, 1, rows.saturating_sub(bar_rows));
                }
            }

            let is_ai = session.is_ai_tool();
            let new_bar_rows = focus_bar_rows(&session.pinned_prompt, is_ai);
            if new_bar_rows != app.bar_rows {
                let old_bar_start = rows.saturating_sub(app.bar_rows) + 1;
                let new_bar_start = rows.saturating_sub(new_bar_rows) + 1;
                ansi::clear_rows(stdout, old_bar_start.min(new_bar_start), rows);
                app.bar_rows = new_bar_rows;
                let term_rows = rows.saturating_sub(new_bar_rows);
                let _ = session.resize(term_rows, cols);
                if !is_alt {
                    ansi::set_scroll_region(stdout, 1, term_rows);
                }
            }

            if !is_alt {
                let bar_rows = app.bar_rows;
                if is_ai {
                    let pin_start = rows - bar_rows + 1;
                    ansi::render_pin_bar(stdout, pin_start, cols, &session.pinned_prompt);
                }
                ansi::render_hint_bar(
                    stdout,
                    rows,
                    app.prefix_armed,
                    &session.window_title(),
                    idx,
                    session_count,
                );
                let (cr, cc) = session.screen().cursor_position();
                write!(stdout, "\x1b[{};{}H", cr + 1, cc + 1).ok();
                stdout.flush().ok();
            }
        }
    }

    // 2. Process non-focused sessions (parser only)
    for (i, session) in app.sessions.iter_mut().enumerate() {
        if i != idx {
            session.process_pty_output();
        }
    }

    // 3. Poll for events
    if let Some(ev) = App::poll_event(Duration::from_millis(16))? {
        match ev {
            Event::Key(key) => {
                handle_focus_key(stdout, app, key, idx)?;
            }
            Event::Paste(text) => {
                if let Some(session) = app.sessions.get_mut(idx) {
                    session.write_bytes(text.as_bytes())?;
                }
            }
            Event::Resize(new_cols, new_rows) => {
                app.rows = new_rows;
                app.cols = new_cols;
                let session_count = app.sessions.len();
                if let Some(session) = app.sessions.get_mut(idx) {
                    let bar_rows = app.bar_rows;
                    let term_rows = new_rows.saturating_sub(bar_rows);
                    let _ = session.resize(term_rows, new_cols);

                    if !session.screen().alternate_screen() {
                        ansi::set_scroll_region(stdout, 1, term_rows);
                        if session.is_ai_tool() {
                            let pin_start = new_rows - bar_rows + 1;
                            ansi::render_pin_bar(
                                stdout,
                                pin_start,
                                new_cols,
                                &session.pinned_prompt,
                            );
                        }
                        ansi::render_hint_bar(
                            stdout,
                            new_rows,
                            app.prefix_armed,
                            &session.window_title(),
                            idx,
                            session_count,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Render the hint bar with current session info (title + index).
fn refresh_hint_bar(stdout: &mut io::Stdout, app: &App, idx: usize, prefix_armed: bool) {
    let title = app.sessions.get(idx).map(|s| s.window_title()).unwrap_or_default();
    ansi::render_hint_bar(stdout, app.rows, prefix_armed, &title, idx, app.sessions.len());
}

/// Handle a key event in Focus mode.
fn handle_focus_key(
    stdout: &mut io::Stdout,
    app: &mut App,
    key: crossterm::event::KeyEvent,
    idx: usize,
) -> Result<()> {
    let is_prefix = key.modifiers.contains(KeyModifiers::CONTROL)
        && (key.code == KeyCode::Char('4') || key.code == KeyCode::Char('\\'));

    if is_prefix {
        app.prefix_armed = true;
        refresh_hint_bar(stdout, app, idx, true);
        return Ok(());
    }

    if app.prefix_armed {
        app.prefix_armed = false;

        match key.code {
            KeyCode::Char('n') => {
                let cwd = app.sessions[idx].cwd.clone();
                match app.create_session(cwd) {
                    Ok(new_idx) => {
                        app.focus_idx = new_idx;
                        setup_focus_mode(stdout, app);
                    }
                    Err(_) => {
                        refresh_hint_bar(stdout, app, idx, false);
                    }
                }
                return Ok(());
            }
            KeyCode::Char('d') => {
                app.delete_current_session();
                if app.should_quit {
                    return Ok(());
                }
                setup_focus_mode(stdout, app);
                return Ok(());
            }
            KeyCode::Char(c @ '1'..='9') => {
                let target = (c as usize) - ('1' as usize);
                if target < app.sessions.len() && target != idx {
                    app.focus_idx = target;
                    setup_focus_mode(stdout, app);
                } else {
                    refresh_hint_bar(stdout, app, idx, false);
                }
                return Ok(());
            }
            KeyCode::Char('q') => {
                app.should_quit = true;
                return Ok(());
            }
            _ => {
                // Unknown prefix key — forward literal Ctrl+\ + the key
                if let Some(session) = app.sessions.get_mut(idx) {
                    session.write_bytes(&[0x1c])?;
                    if let Some(bytes) = key_event_to_bytes(&key) {
                        if let Some(tb) = key_event_to_track_char(&key) {
                            session.track_input(tb);
                        }
                        session.write_bytes(&bytes)?;
                    }
                }
                refresh_hint_bar(stdout, app, idx, false);
                return Ok(());
            }
        }
    }

    // Normal key → forward to PTY
    let session_count = app.sessions.len();
    if let Some(session) = app.sessions.get_mut(idx) {
        if let Some(bytes) = key_event_to_bytes(&key) {
            if let Some(tb) = key_event_to_track_char(&key) {
                session.track_input(tb);
            }
            session.write_bytes(&bytes)?;
        }

        if key.code == KeyCode::Enter {
            let is_ai = session.is_ai_tool();
            let new_bar_rows = focus_bar_rows(&session.pinned_prompt, is_ai);
            if new_bar_rows != app.bar_rows {
                let old_bar_start = app.rows.saturating_sub(app.bar_rows) + 1;
                let new_bar_start = app.rows.saturating_sub(new_bar_rows) + 1;
                ansi::clear_rows(stdout, old_bar_start.min(new_bar_start), app.rows);

                app.bar_rows = new_bar_rows;
                let term_rows = app.rows.saturating_sub(new_bar_rows);
                session.resize(term_rows, app.cols)?;
                if !session.screen().alternate_screen() {
                    ansi::set_scroll_region(stdout, 1, term_rows);
                }
                ansi::render_hint_bar(
                    stdout,
                    app.rows,
                    app.prefix_armed,
                    &session.window_title(),
                    idx,
                    session_count,
                );
            }
            if is_ai {
                let pin_start = app.rows.saturating_sub(app.bar_rows) + 1;
                ansi::render_pin_bar(stdout, pin_start, app.cols, &session.pinned_prompt);
            }
        }
    }

    Ok(())
}

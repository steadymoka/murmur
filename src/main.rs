mod app;
mod key;
mod layout;
mod session;
mod ui;
mod update;

use std::io::{self, Write};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use app::App;
use key::{key_event_to_bytes, key_event_to_track_char};
use layout::focus_bar_rows;
use session::Session;
use ui::ansi;
use ui::bar::{self, BarState};

fn main() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    let _ = crossterm::execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    let cwd = std::env::current_dir()?;
    let (cols, rows) = crossterm::terminal::size()?;

    let mut app = App::new(cwd, rows, cols)?;
    let update_rx = update::check_for_update();

    setup_focus_mode(&mut stdout, &mut app);

    loop {
        poll_update(&mut app, &update_rx);
        let idx = app.focus_idx;
        run_focus_tick(&mut stdout, &mut app, idx)?;
        if app.should_quit {
            break;
        }
    }

    ansi::reset_scroll_region(&mut stdout);
    crossterm::execute!(stdout, crossterm::cursor::Show)?;
    let _ = crossterm::execute!(stdout, PopKeyboardEnhancementFlags);
    disable_raw_mode()?;

    ansi::clear_screen(&mut stdout);
    stdout.flush()?;

    Ok(())
}

fn poll_update(app: &mut App, rx: &mpsc::Receiver<Option<String>>) {
    if app.update_available.is_some() {
        return;
    }
    if let Ok(Some(version)) = rx.try_recv() {
        app.update_available = Some(version);
    }
}

/// Forward a key event to the PTY session, tracking input for pin history.
fn forward_key(session: &mut Session, key: &crossterm::event::KeyEvent) -> Result<()> {
    if let Some(bytes) = key_event_to_bytes(key) {
        if let Some(tb) = key_event_to_track_char(key) {
            session.track_input(tb);
        }
        session.write_bytes(&bytes)?;
    }
    Ok(())
}

/// Render bars and restore cursor to the session's position.
fn render_bars_and_restore_cursor(stdout: &mut io::Stdout, app: &App, idx: usize) {
    render_all_bars(stdout, app, idx);
    restore_session_cursor(stdout, app, idx);
}

/// Render bars using immutable borrows of App + Session.
fn render_all_bars(stdout: &mut io::Stdout, app: &App, idx: usize) {
    if let Some(session) = app.sessions.get(idx) {
        let title = session.window_title();
        let state = BarState {
            rows: app.rows,
            cols: app.cols,
            bar_rows: app.bar_rows,
            is_ai: session::is_ai_tool_title(&title),
            pinned_prompt: session.pins.current(),
            pin_position: session.pins.position(),
            prefix_armed: app.prefix_armed,
            window_title: &title,
            session_index: idx,
            session_count: app.sessions.len(),
            update_version: app.update_available.as_deref(),
        };
        bar::render_bars(stdout, &state);
    }
}

fn restore_session_cursor(stdout: &mut io::Stdout, app: &App, idx: usize) {
    if let Some(session) = app.sessions.get(idx) {
        let (cr, cc) = session.screen().cursor_position();
        ansi::move_to(stdout, cr + 1, cc + 1);
    }
}

/// Set up Focus mode: clear screen, restore PTY contents, set scroll region, render bars.
fn setup_focus_mode(stdout: &mut io::Stdout, app: &mut App) {
    let rows = app.rows;
    let cols = app.cols;
    let idx = app.focus_idx;

    if let Some(session) = app.sessions.get_mut(idx) {
        let is_ai = session.is_ai_tool();
        app.bar_rows = focus_bar_rows(session.pins.current(), is_ai);
        let bar_rows = app.bar_rows;

        let term_rows = rows.saturating_sub(bar_rows);
        let _ = session.resize(term_rows, cols);

        ansi::clear_screen(stdout);

        let contents = session.screen().contents_formatted();
        stdout.write_all(&contents).ok();

        if !session.screen().alternate_screen() {
            ansi::set_scroll_region(stdout, 1, rows.saturating_sub(bar_rows));
        }
    }

    render_bars_and_restore_cursor(stdout, app, idx);
    stdout.flush().ok();
}

/// One tick of the Focus mode loop.
fn run_focus_tick(stdout: &mut io::Stdout, app: &mut App, idx: usize) -> Result<()> {
    let rows = app.rows;
    let cols = app.cols;

    // 1. Drain raw PTY output from the focused session
    let mut has_output = false;
    let mut is_alt = false;
    if let Some(session) = app.sessions.get_mut(idx) {
        let chunks = session.drain_raw_chunks();
        if !chunks.is_empty() {
            has_output = true;
            for chunk in &chunks {
                stdout.write_all(chunk)?;
                session.feed_parser(chunk);
            }
            stdout.flush()?;

            is_alt = session.screen().alternate_screen();
            let is_ai = session.is_ai_tool();

            if let Some(resize) =
                bar::apply_bar_resize(stdout, rows, app.bar_rows, session.pins.current(), is_ai)
            {
                app.bar_rows = resize.new_bar_rows;
                let _ = session.resize(resize.term_rows, cols);
            }
        }
    }

    if has_output {
        if is_alt {
            ansi::reset_scroll_region(stdout);
        } else {
            ansi::set_scroll_region(stdout, 1, rows.saturating_sub(app.bar_rows));
            render_bars_and_restore_cursor(stdout, app, idx);
        }
        stdout.flush().ok();
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

                if let Some(session) = app.sessions.get_mut(idx) {
                    let term_rows = new_rows.saturating_sub(app.bar_rows);
                    let _ = session.resize(term_rows, new_cols);
                }

                if app
                    .sessions
                    .get(idx)
                    .is_some_and(|s| !s.screen().alternate_screen())
                {
                    let term_rows = new_rows.saturating_sub(app.bar_rows);
                    ansi::set_scroll_region(stdout, 1, term_rows);
                    render_bars_and_restore_cursor(stdout, app, idx);
                }
                stdout.flush().ok();
            }
            _ => {}
        }
    }

    Ok(())
}

/// Render the hint bar with current session info.
fn refresh_hint_bar(stdout: &mut io::Stdout, app: &App, idx: usize) {
    let title = app
        .sessions
        .get(idx)
        .map(|s| s.window_title())
        .unwrap_or_default();
    ansi::save_cursor(stdout);
    ansi::render_hint_bar(
        stdout,
        app.rows,
        app.prefix_armed,
        &title,
        idx,
        app.sessions.len(),
        app.update_available.as_deref(),
    );
    ansi::restore_cursor(stdout);
    stdout.flush().ok();
}

/// Re-render the pin bar, handling bar_rows changes.
fn refresh_pin_bar(stdout: &mut io::Stdout, app: &mut App, idx: usize) {
    if !app.sessions[idx].is_ai_tool() {
        return;
    }
    if let Some(resize) = bar::apply_bar_resize(
        stdout,
        app.rows,
        app.bar_rows,
        app.sessions[idx].pins.current(),
        true,
    ) {
        app.bar_rows = resize.new_bar_rows;
        let _ = app.sessions[idx].resize(resize.term_rows, app.cols);
        if !app.sessions[idx].screen().alternate_screen() {
            ansi::set_scroll_region(stdout, 1, resize.term_rows);
        }
    }
    let session = &app.sessions[idx];
    ansi::render_bar_area(
        stdout,
        app.rows,
        app.bar_rows,
        app.cols,
        true,
        session.pins.current(),
        session.pins.position(),
    );
}

/// Move pin cursor forward (next) or backward (prev).
fn navigate_pin(stdout: &mut io::Stdout, app: &mut App, idx: usize, forward: bool) {
    if let Some(session) = app.sessions.get_mut(idx) {
        let changed = if forward {
            session.pins.next()
        } else {
            session.pins.prev()
        };
        if changed && session.is_ai_tool() {
            refresh_pin_bar(stdout, app, idx);
        }
    }
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
        refresh_hint_bar(stdout, app, idx);
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
                        refresh_hint_bar(stdout, app, idx);
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
                    refresh_hint_bar(stdout, app, idx);
                }
                return Ok(());
            }
            KeyCode::Char('x') => {
                if let Some(session) = app.sessions.get_mut(idx) {
                    session.pins.delete();
                    if session.is_ai_tool() {
                        refresh_pin_bar(stdout, app, idx);
                    }
                }
                refresh_hint_bar(stdout, app, idx);
                return Ok(());
            }
            KeyCode::Char('u') => {
                if let Some(ver) = &app.update_available {
                    ansi::save_cursor(stdout);
                    ansi::render_update_message(stdout, app.rows, ver);
                    ansi::restore_cursor(stdout);
                    stdout.flush().ok();
                } else {
                    refresh_hint_bar(stdout, app, idx);
                }
                return Ok(());
            }
            KeyCode::Char('q') => {
                app.should_quit = true;
                return Ok(());
            }
            _ => {
                if let Some(session) = app.sessions.get_mut(idx) {
                    session.write_bytes(&[0x1c])?;
                    forward_key(session, &key)?;
                }
                refresh_hint_bar(stdout, app, idx);
                return Ok(());
            }
        }
    }

    // Direct pin navigation: Ctrl+[ (prev) / Ctrl+] (next)
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('[') => {
                navigate_pin(stdout, app, idx, false);
                return Ok(());
            }
            KeyCode::Char(']') => {
                navigate_pin(stdout, app, idx, true);
                return Ok(());
            }
            _ => {}
        }
    }

    // Normal key → forward to PTY
    let mut enter_resized = false;
    if let Some(session) = app.sessions.get_mut(idx) {
        forward_key(session, &key)?;

        if key.code == KeyCode::Enter {
            if let Some(resize) = bar::apply_bar_resize(
                stdout,
                app.rows,
                app.bar_rows,
                session.pins.current(),
                session.is_ai_tool(),
            ) {
                app.bar_rows = resize.new_bar_rows;
                session.resize(resize.term_rows, app.cols)?;
                if !session.screen().alternate_screen() {
                    ansi::set_scroll_region(stdout, 1, resize.term_rows);
                }
                enter_resized = true;
            }
        }
    }

    if key.code == KeyCode::Enter {
        if enter_resized {
            refresh_hint_bar(stdout, app, idx);
        }
        if let Some(session) = app.sessions.get(idx) {
            ansi::render_bar_area(
                stdout,
                app.rows,
                app.bar_rows,
                app.cols,
                session.is_ai_tool(),
                session.pins.current(),
                session.pins.position(),
            );
        }
        stdout.flush().ok();
    }

    Ok(())
}

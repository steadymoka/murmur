mod app;
mod session;
mod ui;

use std::io::{self, Write};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{focus_bar_rows, key_event_to_bytes, key_event_to_track_char, App, AppState};
use ui::ansi;

fn main() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    let cwd = std::env::current_dir()?;
    let (cols, rows) = crossterm::terminal::size()?;

    let mut app = App::new(cwd, rows, cols)?;

    // Start in Focus mode — set up scroll region + bars
    setup_focus_mode(&mut stdout, &mut app);

    loop {
        match app.state.clone() {
            AppState::Focus(idx) => {
                run_focus_tick(&mut stdout, &mut app, idx)?;
            }
            AppState::Overview => {
                run_overview_loop(&mut app)?;
                if app.should_quit {
                    break;
                }
                // Returned from Overview → now in Focus mode
                continue;
            }
        }
        if app.should_quit {
            break;
        }
    }

    // Cleanup
    ansi::reset_scroll_region(&mut stdout);
    execute!(stdout, crossterm::cursor::Show)?;
    disable_raw_mode()?;

    // Clear screen and move cursor home for clean exit
    write!(stdout, "\x1b[2J\x1b[H")?;
    stdout.flush()?;

    Ok(())
}

/// Set up Focus mode: clear screen, restore PTY contents, set scroll region, render bars.
fn setup_focus_mode(stdout: &mut io::Stdout, app: &mut App) {
    let rows = app.rows;
    let cols = app.cols;

    if let AppState::Focus(idx) = app.state {
        if let Some(session) = app.sessions.get_mut(idx) {
            app.bar_rows = focus_bar_rows(&session.pinned_prompt);
            let bar_rows = app.bar_rows;

            let term_rows = rows.saturating_sub(bar_rows);
            let _ = session.resize(term_rows, cols);

            // Clear screen
            write!(stdout, "\x1b[2J\x1b[H").ok();

            // Restore PTY screen contents
            let contents = session.screen().contents_formatted();
            stdout.write_all(&contents).ok();

            // Set scroll region excluding bottom bar rows
            let scroll_bottom = rows.saturating_sub(bar_rows);
            if !session.screen().alternate_screen() {
                ansi::set_scroll_region(stdout, 1, scroll_bottom);
            }

            // Render bars
            let pin_start = rows - bar_rows + 1;
            let hint_row = rows;
            ansi::render_pin_bar(stdout, pin_start, cols, &session.pinned_prompt);
            ansi::render_hint_bar(stdout, hint_row, app.prefix_armed, &session.window_title());

            // Restore cursor to PTY position
            let (cr, cc) = session.screen().cursor_position();
            write!(stdout, "\x1b[{};{}H", cr + 1, cc + 1).ok();
            stdout.flush().ok();
        }
    }
}

/// One tick of the Focus mode loop.
fn run_focus_tick(stdout: &mut io::Stdout, app: &mut App, idx: usize) -> Result<()> {
    let rows = app.rows;
    let cols = app.cols;

    // 1. Drain raw PTY output from the focused session and write to stdout
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

            // Toggle scroll region on alternate screen transitions
            if was_alt != is_alt {
                if is_alt {
                    ansi::reset_scroll_region(stdout);
                } else {
                    let bar_rows = app.bar_rows;
                    ansi::set_scroll_region(stdout, 1, rows.saturating_sub(bar_rows));
                }
            }

            // Re-render bars after PTY output to keep them visible
            if !is_alt {
                let bar_rows = app.bar_rows;
                let pin_start = rows - bar_rows + 1;
                ansi::render_pin_bar(stdout, pin_start, cols, &session.pinned_prompt);
                ansi::render_hint_bar(stdout, rows, app.prefix_armed, &session.window_title());
                // Restore cursor to where PTY left it
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
                if let Some(session) = app.sessions.get_mut(idx) {
                    let bar_rows = app.bar_rows;
                    let term_rows = new_rows.saturating_sub(bar_rows);
                    let _ = session.resize(term_rows, new_cols);

                    if !session.screen().alternate_screen() {
                        ansi::set_scroll_region(stdout, 1, term_rows);
                        let pin_start = new_rows - bar_rows + 1;
                        let hint_row = new_rows;
                        ansi::render_pin_bar(
                            stdout,
                            pin_start,
                            new_cols,
                            &session.pinned_prompt,
                        );
                        ansi::render_hint_bar(
                            stdout,
                            hint_row,
                            app.prefix_armed,
                            &session.window_title(),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Handle a key event in Focus mode.
fn handle_focus_key(
    stdout: &mut io::Stdout,
    app: &mut App,
    key: crossterm::event::KeyEvent,
    idx: usize,
) -> Result<()> {
    // Ctrl+\ (crossterm maps it to Char('4') + CONTROL on 0.28,
    // and to Char('\\') + CONTROL on 0.29)
    let is_prefix = key.modifiers.contains(KeyModifiers::CONTROL)
        && (key.code == KeyCode::Char('4') || key.code == KeyCode::Char('\\'));

    if is_prefix {
        app.prefix_armed = true;
        let hint_row = app.rows;
        let title = app.sessions.get(idx).map(|s| s.window_title()).unwrap_or_default();
        ansi::render_hint_bar(stdout, hint_row, true, &title);
        return Ok(());
    }

    if app.prefix_armed {
        app.prefix_armed = false;
        let hint_row = app.rows;
        let title = app.sessions.get(idx).map(|s| s.window_title()).unwrap_or_default();
        ansi::render_hint_bar(stdout, hint_row, false, &title);

        match key.code {
            KeyCode::Char('o') => {
                // Transition to Overview
                app.state = AppState::Overview;
                return Ok(());
            }
            KeyCode::Char('q') => {
                app.should_quit = true;
                return Ok(());
            }
            _ => {
                // Forward the literal Ctrl+\ byte + the key
                if let Some(session) = app.sessions.get_mut(idx) {
                    session.write_bytes(&[0x1c])?;
                    if let Some(bytes) = key_event_to_bytes(&key) {
                        if let Some(tb) = key_event_to_track_char(&key) {
                            session.track_input(tb);
                        }
                        session.write_bytes(&bytes)?;
                    }
                }
                return Ok(());
            }
        }
    }

    // Normal key → forward to PTY
    if let Some(session) = app.sessions.get_mut(idx) {
        if let Some(bytes) = key_event_to_bytes(&key) {
            if let Some(tb) = key_event_to_track_char(&key) {
                session.track_input(tb);
            }
            session.write_bytes(&bytes)?;
        }

        // Update bars if enter was pressed (pinned_prompt may have changed)
        if key.code == KeyCode::Enter {
            let new_bar_rows = focus_bar_rows(&session.pinned_prompt);
            if new_bar_rows != app.bar_rows {
                // Clear old bar area
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
                );
            }
            let pin_start = app.rows.saturating_sub(app.bar_rows) + 1;
            ansi::render_pin_bar(stdout, pin_start, app.cols, &session.pinned_prompt);
        }
    }

    Ok(())
}

/// Run the Overview loop. Returns when the user transitions back to Focus or quits.
fn run_overview_loop(app: &mut App) -> Result<()> {
    let mut stdout = io::stdout();

    // Transition: reset scroll region → enter alternate screen
    ansi::reset_scroll_region(&mut stdout);
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        app.process_all_sessions();
        terminal.draw(|frame| app.draw_overview(frame))?;

        if let Some(ev) = App::poll_event(Duration::from_millis(16))? {
            match ev {
                Event::Key(key) => {
                    let prev_state = app.state.clone();
                    app.handle_overview_key(key)?;

                    // If state changed to Focus, exit Overview loop
                    if app.state != prev_state {
                        if let AppState::Focus(_) = app.state {
                            break;
                        }
                    }
                }
                Event::Resize(cols, rows) => {
                    app.rows = rows;
                    app.cols = cols;
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Leave alternate screen
    drop(terminal);
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen)?;

    // If transitioning to Focus, restore PTY contents + scroll region
    if !app.should_quit {
        setup_focus_mode(&mut stdout, app);
    }

    Ok(())
}

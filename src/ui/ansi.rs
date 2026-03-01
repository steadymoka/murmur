use std::io::{self, Write};

/// Set DECSTBM scroll region to rows [top, bottom] (1-indexed).
pub fn set_scroll_region(stdout: &mut io::Stdout, top: u16, bottom: u16) {
    write!(stdout, "\x1b[{};{}r", top, bottom).ok();
    stdout.flush().ok();
}

/// Reset DECSTBM scroll region to full screen.
pub fn reset_scroll_region(stdout: &mut io::Stdout) {
    write!(stdout, "\x1b[r").ok();
    stdout.flush().ok();
}

pub fn save_cursor(stdout: &mut io::Stdout) {
    write!(stdout, "\x1b7").ok();
}

pub fn restore_cursor(stdout: &mut io::Stdout) {
    write!(stdout, "\x1b8").ok();
}

/// Move cursor to (row, col), 1-indexed.
fn move_to(stdout: &mut io::Stdout, row: u16, col: u16) {
    write!(stdout, "\x1b[{};{}H", row, col).ok();
}

/// Clear the entire line the cursor is on.
fn clear_line(stdout: &mut io::Stdout) {
    write!(stdout, "\x1b[2K").ok();
}

/// Clear rows [from, to] inclusive (1-indexed).
pub fn clear_rows(stdout: &mut io::Stdout, from: u16, to: u16) {
    save_cursor(stdout);
    for row in from..=to {
        move_to(stdout, row, 1);
        clear_line(stdout);
    }
    restore_cursor(stdout);
    stdout.flush().ok();
}

/// Render the PIN bar starting at `start_row` (1-indexed).
/// Uses a left-bar chat style: ▎ line
/// `position` is Some((current_1based, total)) when navigating history.
pub fn render_pin_bar(
    stdout: &mut io::Stdout,
    start_row: u16,
    cols: u16,
    pinned_prompt: &str,
    position: Option<(usize, usize)>,
) {
    save_cursor(stdout);

    if pinned_prompt.is_empty() {
        move_to(stdout, start_row, 1);
        clear_line(stdout);
        write!(stdout, "\x1b[90m \u{258e} (no prompt)\x1b[0m").ok();
    } else {
        let indicator = match position {
            Some((cur, total)) => format!("[{}/{}] ", cur, total),
            None => String::new(),
        };
        let indicator_width = indicator.len();
        let available = (cols as usize).saturating_sub(4 + indicator_width);
        let lines: Vec<&str> = pinned_prompt.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            let row = start_row + i as u16;
            move_to(stdout, row, 1);
            clear_line(stdout);

            let display = if line.len() > available {
                format!("{}...", &line[..available.saturating_sub(3)])
            } else {
                (*line).to_string()
            };

            if i == 0 && !indicator.is_empty() {
                write!(
                    stdout,
                    "\x1b[36m \u{258e}\x1b[0m \x1b[90m{}\x1b[33m{}\x1b[0m",
                    indicator, display
                )
                .ok();
            } else {
                write!(
                    stdout,
                    "\x1b[36m \u{258e}\x1b[0m \x1b[33m{}\x1b[0m",
                    display
                )
                .ok();
            }
        }
    }

    restore_cursor(stdout);
    stdout.flush().ok();
}

/// Render the hint bar at the given row (1-indexed).
pub fn render_hint_bar(
    stdout: &mut io::Stdout,
    row: u16,
    prefix_armed: bool,
    window_title: &str,
    session_index: usize,
    session_count: usize,
) {
    save_cursor(stdout);
    move_to(stdout, row, 1);
    clear_line(stdout);

    if prefix_armed {
        write!(
            stdout,
            "\x1b[1;30;46m Ctrl+\\ \x1b[0;36m n: new  d: del  []: pin  x: unpin  1-9: switch  q: quit \x1b[0m"
        )
        .ok();
    } else {
        if session_count > 1 {
            write!(
                stdout,
                "\x1b[36m[{}/{}]\x1b[0m ",
                session_index + 1,
                session_count
            )
            .ok();
        }

        if !window_title.is_empty() {
            write!(stdout, "\x1b[90m{}\x1b[0m", window_title).ok();
        }

        write!(
            stdout,
            "\x1b[90m \u{2502} \x1b[36mCtrl+\\\x1b[90m \u{2192} n/d/q\x1b[0m"
        )
        .ok();
    }

    restore_cursor(stdout);
    stdout.flush().ok();
}

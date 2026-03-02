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

const BAR_BG: &str = "\x1b[48;5;236m";

/// Render a dim horizontal separator line at the given row.
fn render_separator(stdout: &mut io::Stdout, row: u16, cols: u16) {
    move_to(stdout, row, 1);
    let line: String = "\u{2500}".repeat(cols as usize);
    write!(stdout, "\x1b[90m{line}\x1b[K\x1b[0m").ok();
}

/// Render the separator and, when the session is an AI tool, the pin bar below it.
pub fn render_bar_area(
    stdout: &mut io::Stdout,
    rows: u16,
    bar_rows: u16,
    cols: u16,
    is_ai: bool,
    pinned_prompt: &str,
    position: Option<(usize, usize)>,
) {
    save_cursor(stdout);
    let separator_row = rows.saturating_sub(bar_rows) + 1;
    render_separator(stdout, separator_row, cols);
    if is_ai {
        render_pin_bar(stdout, separator_row + 1, cols, pinned_prompt, position);
    }
    restore_cursor(stdout);
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
        write!(stdout, "{BAR_BG}\x1b[90m \u{258e} (no prompt)\x1b[K\x1b[0m").ok();
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
                    "{BAR_BG}\x1b[36m \u{258e}\x1b[0m{BAR_BG} \x1b[90m{}\x1b[33m{}\x1b[K\x1b[0m",
                    indicator, display
                )
                .ok();
            } else {
                write!(
                    stdout,
                    "{BAR_BG}\x1b[36m \u{258e}\x1b[0m{BAR_BG} \x1b[33m{}\x1b[K\x1b[0m",
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
    update_version: Option<&str>,
) {
    save_cursor(stdout);
    move_to(stdout, row, 1);
    clear_line(stdout);

    if prefix_armed {
        let update_hint = if update_version.is_some() {
            "  u: update"
        } else {
            ""
        };
        write!(
            stdout,
            "\x1b[1;30;46m Ctrl+\\ \x1b[0;36m n: new  d: del  []: pin  x: unpin  1-9: switch{update_hint}  q: quit \x1b[0m"
        )
        .ok();
    } else {
        write!(stdout, "{BAR_BG}").ok();

        if session_count > 1 {
            write!(
                stdout,
                "\x1b[36m[{}/{}]\x1b[0m{BAR_BG} ",
                session_index + 1,
                session_count
            )
            .ok();
        }

        if !window_title.is_empty() {
            write!(stdout, "\x1b[90m{}\x1b[0m{BAR_BG}", window_title).ok();
        }

        write!(
            stdout,
            "\x1b[90m \u{2502} \x1b[36mCtrl+\\\x1b[90m \u{2192} n/d/q\x1b[0m{BAR_BG}"
        )
        .ok();

        if let Some(ver) = update_version {
            write!(
                stdout,
                "\x1b[90m \u{2502} \x1b[32m\u{2191} v{ver} available\x1b[0m{BAR_BG}"
            )
            .ok();
        }

        write!(stdout, "\x1b[K\x1b[0m").ok();
    }

    restore_cursor(stdout);
    stdout.flush().ok();
}

/// Render a one-shot update instruction message on the hint bar row.
pub fn render_update_message(stdout: &mut io::Stdout, row: u16, version: &str) {
    save_cursor(stdout);
    move_to(stdout, row, 1);
    clear_line(stdout);
    write!(
        stdout,
        "{BAR_BG}\x1b[32m Update to v{version}: \x1b[1mnpm i -g murmur-tui\x1b[K\x1b[0m"
    )
    .ok();
    restore_cursor(stdout);
    stdout.flush().ok();
}

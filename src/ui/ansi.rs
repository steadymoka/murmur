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

/// Render the PIN bar starting at `start_row` (1-indexed) with yellow bold text.
/// Multiline pinned_prompt renders each line on its own row.
pub fn render_pin_bar(stdout: &mut io::Stdout, start_row: u16, cols: u16, pinned_prompt: &str) {
    save_cursor(stdout);

    if pinned_prompt.is_empty() {
        move_to(stdout, start_row, 1);
        clear_line(stdout);
        write!(stdout, "\x1b[1;33m PIN: (none)\x1b[0m").ok();
    } else {
        let lines: Vec<&str> = pinned_prompt.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            let row = start_row + i as u16;
            move_to(stdout, row, 1);
            clear_line(stdout);

            let is_last = i == lines.len() - 1;
            let (prefix, suffix) = match (i, is_last) {
                (0, true) => (" PIN: \u{201c}", "\u{201d}"),
                (0, false) => (" PIN: \u{201c}", ""),
                (_, true) => ("       ", "\u{201d}"),
                (_, false) => ("       ", ""),
            };

            let available = (cols as usize).saturating_sub(prefix.len() + suffix.len() + 1);
            let text = if line.len() > available {
                format!(
                    "{}{}...{}",
                    prefix,
                    &line[..available.saturating_sub(3)],
                    suffix
                )
            } else {
                format!("{}{}{}", prefix, line, suffix)
            };

            write!(stdout, "\x1b[1;33m{}\x1b[0m", text).ok();
        }
    }

    restore_cursor(stdout);
    stdout.flush().ok();
}

/// Render the hint bar at the given row (1-indexed).
pub fn render_hint_bar(stdout: &mut io::Stdout, row: u16, prefix_armed: bool) {
    save_cursor(stdout);
    move_to(stdout, row, 1);
    clear_line(stdout);

    if prefix_armed {
        // Highlighted: cyan background black text
        write!(
            stdout,
            "\x1b[1;30;46m Ctrl+\\ \x1b[0;36m o: overview  q: quit \x1b[0m"
        )
        .ok();
    } else {
        // Normal: cyan key, dark gray separator
        write!(
            stdout,
            "\x1b[36m Ctrl+\\\x1b[90m \u{2192} \x1b[36mo\x1b[0m: overview"
        )
        .ok();
    }

    restore_cursor(stdout);
    stdout.flush().ok();
}

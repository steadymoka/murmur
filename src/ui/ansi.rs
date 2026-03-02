use std::io::Write;
use unicode_width::UnicodeWidthChar;

/// Set DECSTBM scroll region to rows [top, bottom] (1-indexed).
pub fn set_scroll_region(w: &mut impl Write, top: u16, bottom: u16) {
    write!(w, "\x1b[{};{}r", top, bottom).ok();
}

/// Reset DECSTBM scroll region to full screen.
pub fn reset_scroll_region(w: &mut impl Write) {
    write!(w, "\x1b[r").ok();
}

pub fn save_cursor(w: &mut impl Write) {
    write!(w, "\x1b7").ok();
}

pub fn restore_cursor(w: &mut impl Write) {
    write!(w, "\x1b8").ok();
}

/// Move cursor to (row, col), 1-indexed.
pub fn move_to(w: &mut impl Write, row: u16, col: u16) {
    write!(w, "\x1b[{};{}H", row, col).ok();
}

/// Clear the entire line the cursor is on.
fn clear_line(w: &mut impl Write) {
    write!(w, "\x1b[2K").ok();
}

/// Clear entire screen and move cursor to top-left.
pub fn clear_screen(w: &mut impl Write) {
    write!(w, "\x1b[2J\x1b[H").ok();
}

/// Clear rows [from, to] inclusive (1-indexed).
pub fn clear_rows(w: &mut impl Write, from: u16, to: u16) {
    save_cursor(w);
    for row in from..=to {
        move_to(w, row, 1);
        clear_line(w);
    }
    restore_cursor(w);
}

/// Truncate a string to fit within `max_width` display columns.
pub(crate) fn truncate_to_width(s: &str, max_width: usize) -> &str {
    let mut width = 0;
    for (i, c) in s.char_indices() {
        let cw = c.width().unwrap_or(0);
        if width + cw > max_width {
            return &s[..i];
        }
        width += cw;
    }
    s
}

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[90m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const BOLD: &str = "\x1b[1m";
const CLEAR_EOL: &str = "\x1b[K";
const BAR_BG: &str = "\x1b[48;5;236m";
const PREFIX_STYLE: &str = "\x1b[1;30;46m";
const PREFIX_KEY_STYLE: &str = "\x1b[0;36m";

fn render_separator(w: &mut impl Write, row: u16, cols: u16) {
    move_to(w, row, 1);
    let line = "\u{2500}".repeat(cols as usize);
    write!(w, "{DIM}{line}{CLEAR_EOL}{RESET}").ok();
}

pub fn render_bar_area(
    w: &mut impl Write,
    rows: u16,
    bar_rows: u16,
    cols: u16,
    is_ai: bool,
    pinned_prompt: &str,
    position: Option<(usize, usize)>,
) {
    save_cursor(w);
    let separator_row = rows.saturating_sub(bar_rows) + 1;
    render_separator(w, separator_row, cols);
    if is_ai {
        render_pin_bar(w, separator_row + 1, cols, pinned_prompt, position);
    }
    restore_cursor(w);
}

pub fn render_pin_bar(
    w: &mut impl Write,
    start_row: u16,
    cols: u16,
    pinned_prompt: &str,
    position: Option<(usize, usize)>,
) {
    if pinned_prompt.is_empty() {
        move_to(w, start_row, 1);
        clear_line(w);
        write!(w, "{BAR_BG}{DIM} \u{258e} (no prompt){CLEAR_EOL}{RESET}").ok();
    } else {
        let indicator = match position {
            Some((cur, total)) => format!("[{}/{}] ", cur, total),
            None => String::new(),
        };
        let indicator_width = indicator.len();
        let available = (cols as usize).saturating_sub(4 + indicator_width);
        for (i, line) in pinned_prompt.split('\n').enumerate() {
            let row = start_row + i as u16;
            move_to(w, row, 1);
            clear_line(w);

            let truncated = truncate_to_width(line, available);
            let display = if truncated.len() < line.len() {
                format!(
                    "{}...",
                    truncate_to_width(line, available.saturating_sub(3))
                )
            } else {
                truncated.to_string()
            };

            if i == 0 && !indicator.is_empty() {
                write!(
                    w,
                    "{BAR_BG}{CYAN} \u{258e}{RESET}{BAR_BG} {DIM}{}{YELLOW}{}{CLEAR_EOL}{RESET}",
                    indicator, display
                )
                .ok();
            } else {
                write!(
                    w,
                    "{BAR_BG}{CYAN} \u{258e}{RESET}{BAR_BG} {YELLOW}{}{CLEAR_EOL}{RESET}",
                    display
                )
                .ok();
            }
        }
    }
}

pub fn render_hint_bar(
    w: &mut impl Write,
    row: u16,
    prefix_armed: bool,
    window_title: &str,
    session_index: usize,
    session_count: usize,
    update_version: Option<&str>,
) {
    move_to(w, row, 1);
    clear_line(w);

    if prefix_armed {
        let update_hint = if update_version.is_some() {
            "  u: update"
        } else {
            ""
        };
        write!(
            w,
            "{PREFIX_STYLE} Ctrl+\\ {PREFIX_KEY_STYLE} n: new  d: del  []: pin  x: unpin  1-9: switch{update_hint}  q: quit {RESET}"
        )
        .ok();
    } else {
        write!(w, "{BAR_BG}").ok();

        if session_count > 1 {
            write!(
                w,
                "{CYAN}[{}/{}]{RESET}{BAR_BG} ",
                session_index + 1,
                session_count
            )
            .ok();
        }

        if !window_title.is_empty() {
            write!(w, "{DIM}{}{RESET}{BAR_BG}", window_title).ok();
        }

        write!(
            w,
            "{DIM} \u{2502} {CYAN}Ctrl+\\{DIM} \u{2192} n/d/q{RESET}{BAR_BG}"
        )
        .ok();

        if let Some(ver) = update_version {
            write!(
                w,
                "{DIM} \u{2502} {GREEN}\u{2191} v{ver} available{RESET}{BAR_BG}"
            )
            .ok();
        }

        write!(w, "{CLEAR_EOL}{RESET}").ok();
    }
}

pub fn render_update_message(w: &mut impl Write, row: u16, version: &str) {
    move_to(w, row, 1);
    clear_line(w);
    write!(
        w,
        "{BAR_BG}{GREEN} Update to v{version}: {BOLD}npm i -g murmur-tui{CLEAR_EOL}{RESET}"
    )
    .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(buf: &[u8]) -> String {
        String::from_utf8_lossy(buf).to_string()
    }

    #[test]
    fn test_set_scroll_region() {
        let mut buf = Vec::new();
        set_scroll_region(&mut buf, 1, 20);
        assert_eq!(output(&buf), "\x1b[1;20r");
    }

    #[test]
    fn test_reset_scroll_region() {
        let mut buf = Vec::new();
        reset_scroll_region(&mut buf);
        assert_eq!(output(&buf), "\x1b[r");
    }

    #[test]
    fn test_save_cursor() {
        let mut buf = Vec::new();
        save_cursor(&mut buf);
        assert_eq!(output(&buf), "\x1b7");
    }

    #[test]
    fn test_restore_cursor() {
        let mut buf = Vec::new();
        restore_cursor(&mut buf);
        assert_eq!(output(&buf), "\x1b8");
    }

    #[test]
    fn test_clear_rows() {
        let mut buf = Vec::new();
        clear_rows(&mut buf, 5, 6);
        let s = output(&buf);
        assert!(s.contains("\x1b7")); // save cursor
        assert!(s.contains("\x1b8")); // restore cursor
        assert!(s.contains("\x1b[5;1H")); // move to row 5
        assert!(s.contains("\x1b[6;1H")); // move to row 6
        assert!(s.contains("\x1b[2K")); // clear line
    }

    #[test]
    fn test_truncate_to_width_ascii() {
        assert_eq!(truncate_to_width("hello", 3), "hel");
    }

    #[test]
    fn test_truncate_to_width_fits() {
        assert_eq!(truncate_to_width("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_to_width_exact() {
        assert_eq!(truncate_to_width("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_to_width_cjk() {
        // CJK chars are 2 columns wide
        let s = "\u{4F60}\u{597D}"; // 你好 = 4 columns
        assert_eq!(truncate_to_width(s, 3), "\u{4F60}"); // only 你 fits (2 cols)
        assert_eq!(truncate_to_width(s, 4), s); // both fit
    }

    #[test]
    fn test_truncate_to_width_empty() {
        assert_eq!(truncate_to_width("", 5), "");
    }

    #[test]
    fn test_render_separator() {
        let mut buf = Vec::new();
        render_separator(&mut buf, 10, 5);
        let s = output(&buf);
        assert!(s.contains("\x1b[10;1H")); // move to row 10
        assert!(s.contains("\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}")); // 5 horizontal lines
    }

    #[test]
    fn test_render_pin_bar_empty() {
        let mut buf = Vec::new();
        render_pin_bar(&mut buf, 10, 80, "", None);
        let s = output(&buf);
        assert!(s.contains("(no prompt)"));
    }

    #[test]
    fn test_render_pin_bar_with_content() {
        let mut buf = Vec::new();
        render_pin_bar(&mut buf, 10, 80, "test prompt", None);
        let s = output(&buf);
        assert!(s.contains("test prompt"));
        assert!(s.contains("\u{258e}")); // left bar char
    }

    #[test]
    fn test_render_pin_bar_with_position() {
        let mut buf = Vec::new();
        render_pin_bar(&mut buf, 10, 80, "prompt", Some((2, 5)));
        let s = output(&buf);
        assert!(s.contains("[2/5]"));
        assert!(s.contains("prompt"));
    }

    #[test]
    fn test_render_pin_bar_multiline() {
        let mut buf = Vec::new();
        render_pin_bar(&mut buf, 10, 80, "line1\nline2", None);
        let s = output(&buf);
        assert!(s.contains("line1"));
        assert!(s.contains("line2"));
        assert!(s.contains("\x1b[10;1H")); // first line
        assert!(s.contains("\x1b[11;1H")); // second line
    }

    #[test]
    fn test_render_hint_bar_normal() {
        let mut buf = Vec::new();
        render_hint_bar(&mut buf, 24, false, "my-title", 0, 1, None);
        let s = output(&buf);
        assert!(s.contains("my-title"));
        assert!(s.contains("Ctrl+\\"));
    }

    #[test]
    fn test_render_hint_bar_prefix_armed() {
        let mut buf = Vec::new();
        render_hint_bar(&mut buf, 24, true, "", 0, 1, None);
        let s = output(&buf);
        assert!(s.contains("n: new"));
        assert!(s.contains("d: del"));
        assert!(s.contains("q: quit"));
    }

    #[test]
    fn test_render_hint_bar_with_update() {
        let mut buf = Vec::new();
        render_hint_bar(&mut buf, 24, false, "", 0, 1, Some("0.2.0"));
        let s = output(&buf);
        assert!(s.contains("v0.2.0 available"));
    }

    #[test]
    fn test_render_hint_bar_multi_session() {
        let mut buf = Vec::new();
        render_hint_bar(&mut buf, 24, false, "", 2, 5, None);
        let s = output(&buf);
        assert!(s.contains("[3/5]")); // session_index + 1
    }

    #[test]
    fn test_render_update_message() {
        let mut buf = Vec::new();
        render_update_message(&mut buf, 24, "0.3.0");
        let s = output(&buf);
        assert!(s.contains("v0.3.0"));
        assert!(s.contains("npm i -g murmur-tui"));
    }

    #[test]
    fn test_render_bar_area_non_ai() {
        let mut buf = Vec::new();
        render_bar_area(&mut buf, 24, 2, 80, false, "ignored", None);
        let s = output(&buf);
        // Should have separator but no pin bar
        assert!(s.contains("\u{2500}")); // separator
        assert!(!s.contains("\u{258e}")); // no pin bar char
    }

    #[test]
    fn test_render_bar_area_ai() {
        let mut buf = Vec::new();
        render_bar_area(&mut buf, 24, 3, 80, true, "prompt", None);
        let s = output(&buf);
        assert!(s.contains("\u{2500}")); // separator
        assert!(s.contains("\u{258e}")); // pin bar char
        assert!(s.contains("prompt"));
    }
}

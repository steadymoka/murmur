/// Compute how many rows the bottom bar area occupies (separator + PIN lines + hint bar).
pub fn focus_bar_rows(pinned_prompt: &str, is_ai_tool: bool) -> u16 {
    if !is_ai_tool {
        return 2; // separator + hint bar
    }
    let pin_lines = if pinned_prompt.is_empty() {
        1
    } else {
        (pinned_prompt.bytes().filter(|&b| b == b'\n').count() + 1) as u16
    };
    pin_lines + 2 // separator + pin lines + hint bar
}

/// Result of a bar height change, describing which rows to clear and the new terminal height.
pub struct BarResize {
    pub new_bar_rows: u16,
    pub clear_from: u16,
    pub clear_to: u16,
    pub term_rows: u16,
}

/// Compute bar resize parameters when bar height changes.
/// Returns `None` if bar height did not change.
pub fn compute_bar_resize(
    rows: u16,
    old_bar_rows: u16,
    pinned_prompt: &str,
    is_ai_tool: bool,
) -> Option<BarResize> {
    let new_bar_rows = focus_bar_rows(pinned_prompt, is_ai_tool);
    if new_bar_rows == old_bar_rows {
        return None;
    }
    let old_bar_start = rows.saturating_sub(old_bar_rows) + 1;
    let new_bar_start = rows.saturating_sub(new_bar_rows) + 1;
    Some(BarResize {
        new_bar_rows,
        clear_from: old_bar_start.min(new_bar_start),
        clear_to: rows,
        term_rows: rows.saturating_sub(new_bar_rows),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_rows_non_ai() {
        assert_eq!(focus_bar_rows("anything", false), 2);
        assert_eq!(focus_bar_rows("", false), 2);
    }

    #[test]
    fn bar_rows_ai_empty_pin() {
        assert_eq!(focus_bar_rows("", true), 3);
    }

    #[test]
    fn bar_rows_ai_single_line() {
        assert_eq!(focus_bar_rows("hello world", true), 3);
    }

    #[test]
    fn bar_rows_ai_multiline() {
        assert_eq!(focus_bar_rows("line1\nline2\nline3", true), 5);
    }

    #[test]
    fn resize_no_change() {
        let result = compute_bar_resize(40, 2, "", false);
        assert!(result.is_none());
    }

    #[test]
    fn resize_grow() {
        // non-AI (2 rows) → AI with pin (3 rows)
        let result = compute_bar_resize(40, 2, "prompt", true).unwrap();
        assert_eq!(result.new_bar_rows, 3);
        assert_eq!(result.term_rows, 37);
        assert_eq!(result.clear_from, 38); // min(39, 38)
        assert_eq!(result.clear_to, 40);
    }

    #[test]
    fn resize_shrink() {
        // AI multiline (5 rows) → AI single line (3 rows)
        let result = compute_bar_resize(40, 5, "single", true).unwrap();
        assert_eq!(result.new_bar_rows, 3);
        assert_eq!(result.term_rows, 37);
        assert_eq!(result.clear_from, 36); // min(36, 38)
        assert_eq!(result.clear_to, 40);
    }

    #[test]
    fn resize_term_rows() {
        let result = compute_bar_resize(24, 2, "a\nb", true).unwrap();
        assert_eq!(result.new_bar_rows, 4);
        assert_eq!(result.term_rows, 20);
    }
}

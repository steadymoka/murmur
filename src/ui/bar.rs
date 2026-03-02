use std::io::Write;

use super::ansi;
use crate::layout::{self, BarResize};

/// All data needed to render the bar area. Gathered from App + Session
/// to avoid passing mutable references across boundaries.
pub struct BarState<'a> {
    pub rows: u16,
    pub cols: u16,
    pub bar_rows: u16,
    pub is_ai: bool,
    pub pinned_prompt: &'a str,
    pub pin_position: Option<(usize, usize)>,
    pub prefix_armed: bool,
    pub window_title: &'a str,
    pub session_index: usize,
    pub session_count: usize,
    pub update_version: Option<&'a str>,
}

/// Full bar redraw: separator + pin bar + hint bar.
pub fn render_bars(w: &mut impl Write, state: &BarState) {
    ansi::render_bar_area(
        w,
        state.rows,
        state.bar_rows,
        state.cols,
        state.is_ai,
        state.pinned_prompt,
        state.pin_position,
    );
    ansi::render_hint_bar(
        w,
        state.rows,
        state.prefix_armed,
        state.window_title,
        state.session_index,
        state.session_count,
        state.update_version,
    );
}

/// Handle bar resize: clear old rows, compute new layout.
/// Returns the resize info if a resize occurred, `None` otherwise.
pub fn apply_bar_resize(
    w: &mut impl Write,
    rows: u16,
    old_bar_rows: u16,
    pinned_prompt: &str,
    is_ai: bool,
) -> Option<BarResize> {
    let resize = layout::compute_bar_resize(rows, old_bar_rows, pinned_prompt, is_ai)?;
    ansi::clear_rows(w, resize.clear_from, resize.clear_to);
    Some(resize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_bars_writes_output() {
        let mut buf = Vec::new();
        let state = BarState {
            rows: 24,
            cols: 80,
            bar_rows: 3,
            is_ai: true,
            pinned_prompt: "test",
            pin_position: None,
            prefix_armed: false,
            window_title: "title",
            session_index: 0,
            session_count: 1,
            update_version: None,
        };
        render_bars(&mut buf, &state);
        assert!(!buf.is_empty());
        let s = String::from_utf8_lossy(&buf);
        assert!(s.contains("test")); // pin bar content
        assert!(s.contains("title")); // hint bar content
    }

    #[test]
    fn apply_bar_resize_no_change() {
        let mut buf = Vec::new();
        let result = apply_bar_resize(&mut buf, 24, 2, "", false);
        assert!(result.is_none());
        assert!(buf.is_empty());
    }

    #[test]
    fn apply_bar_resize_clears_rows() {
        let mut buf = Vec::new();
        let result = apply_bar_resize(&mut buf, 24, 2, "prompt", true);
        assert!(result.is_some());
        let resize = result.unwrap();
        assert_eq!(resize.new_bar_rows, 3);
        assert!(!buf.is_empty()); // clear_rows wrote something
    }
}

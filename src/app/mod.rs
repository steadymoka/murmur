use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event};

use crate::layout::focus_bar_rows;
use crate::session::Session;

pub struct App {
    pub sessions: Vec<Session>,
    pub should_quit: bool,
    pub prefix_armed: bool,
    pub bar_rows: u16,
    pub rows: u16,
    pub cols: u16,
    pub focus_idx: usize,
    pub update_available: Option<String>,
}

impl App {
    pub fn new(cwd: PathBuf, rows: u16, cols: u16) -> Result<Self> {
        let bar_rows = focus_bar_rows("", false);
        let term_rows = rows.saturating_sub(bar_rows);
        let session = Session::spawn(cwd, term_rows, cols)?;

        Ok(Self {
            sessions: vec![session],
            should_quit: false,
            prefix_armed: false,
            bar_rows,
            rows,
            cols,
            focus_idx: 0,
            update_available: None,
        })
    }

    pub fn create_session(&mut self, cwd: PathBuf) -> Result<usize> {
        let term_rows = self.rows.saturating_sub(self.bar_rows);
        let session = Session::spawn(cwd, term_rows, self.cols)?;
        self.sessions.push(session);
        let idx = self.sessions.len() - 1;
        Ok(idx)
    }

    pub fn delete_current_session(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let old_len = self.sessions.len();
        self.sessions.remove(self.focus_idx);
        match adjust_focus_after_delete(self.focus_idx, old_len) {
            Some(new_idx) => self.focus_idx = new_idx,
            None => self.should_quit = true,
        }
    }

    pub fn poll_event(timeout: std::time::Duration) -> Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }
}

/// Compute the new focus_idx after removing a session.
/// Returns `None` if removal leaves no sessions (should quit).
pub fn adjust_focus_after_delete(focus_idx: usize, old_len: usize) -> Option<usize> {
    if old_len <= 1 {
        return None;
    }
    let new_len = old_len - 1;
    if focus_idx >= new_len {
        Some(new_len - 1)
    } else {
        Some(focus_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjust_focus_single_session() {
        assert_eq!(adjust_focus_after_delete(0, 1), None);
    }

    #[test]
    fn adjust_focus_delete_last() {
        // 3 sessions, focus on last (idx=2), after delete: idx=1
        assert_eq!(adjust_focus_after_delete(2, 3), Some(1));
    }

    #[test]
    fn adjust_focus_delete_middle() {
        // 3 sessions, focus on middle (idx=1), after delete: idx stays 1
        assert_eq!(adjust_focus_after_delete(1, 3), Some(1));
    }

    #[test]
    fn adjust_focus_delete_first() {
        // 3 sessions, focus on first (idx=0), after delete: idx stays 0
        assert_eq!(adjust_focus_after_delete(0, 3), Some(0));
    }
}

use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::session::Session;

/// Compute how many rows the bottom bar area occupies (PIN lines + hint bar).
pub fn focus_bar_rows(pinned_prompt: &str, is_ai_tool: bool) -> u16 {
    if !is_ai_tool {
        return 1; // hint bar only
    }
    let pin_lines = if pinned_prompt.is_empty() {
        1
    } else {
        (pinned_prompt.chars().filter(|&c| c == '\n').count() + 1) as u16
    };
    pin_lines + 1
}

pub struct App {
    pub sessions: Vec<Session>,
    pub should_quit: bool,
    pub prefix_armed: bool,
    pub bar_rows: u16,
    pub rows: u16,
    pub cols: u16,
    pub focus_idx: usize,
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
        self.sessions.remove(self.focus_idx);
        if self.sessions.is_empty() {
            self.should_quit = true;
            return;
        }
        if self.focus_idx >= self.sessions.len() {
            self.focus_idx = self.sessions.len() - 1;
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

/// Convert a crossterm KeyEvent to raw bytes suitable for PTY input.
/// Supports Alt modifier (prepends ESC), UTF-8 chars, control bytes,
/// special keys, and F1-F12.
pub fn key_event_to_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    let mut bytes = match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let b = c.to_ascii_lowercase() as u8;
                if b.is_ascii_lowercase() {
                    vec![b - b'a' + 1]
                } else {
                    return None;
                }
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(1) => b"\x1bOP".to_vec(),
        KeyCode::F(2) => b"\x1bOQ".to_vec(),
        KeyCode::F(3) => b"\x1bOR".to_vec(),
        KeyCode::F(4) => b"\x1bOS".to_vec(),
        KeyCode::F(5) => b"\x1b[15~".to_vec(),
        KeyCode::F(6) => b"\x1b[17~".to_vec(),
        KeyCode::F(7) => b"\x1b[18~".to_vec(),
        KeyCode::F(8) => b"\x1b[19~".to_vec(),
        KeyCode::F(9) => b"\x1b[20~".to_vec(),
        KeyCode::F(10) => b"\x1b[21~".to_vec(),
        KeyCode::F(11) => b"\x1b[23~".to_vec(),
        KeyCode::F(12) => b"\x1b[24~".to_vec(),
        _ => return None,
    };

    if alt {
        bytes.insert(0, 0x1b);
    }

    Some(bytes)
}

/// Extract a char for input tracking (all printable chars including CJK, control, backspace).
pub fn key_event_to_track_char(key: &KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let b = c.to_ascii_lowercase() as u8;
                if b.is_ascii_lowercase() {
                    Some((b - b'a' + 1) as char)
                } else {
                    None
                }
            } else {
                Some(c)
            }
        }
        KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                Some('\n')
            } else {
                Some('\r')
            }
        }
        KeyCode::Backspace => Some('\x7f'),
        _ => None,
    }
}

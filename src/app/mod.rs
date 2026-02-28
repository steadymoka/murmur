use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;

use crate::session::Session;
use crate::ui;

/// Compute how many rows the bottom bar area occupies (PIN lines + hint bar).
pub fn focus_bar_rows(pinned_prompt: &str) -> u16 {
    let pin_lines = if pinned_prompt.is_empty() {
        1
    } else {
        (pinned_prompt.chars().filter(|&c| c == '\n').count() + 1) as u16
    };
    pin_lines + 1
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Overview,
    Focus(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    NewSession(String),
}

pub struct App {
    pub state: AppState,
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub input_mode: InputMode,
    pub should_quit: bool,
    pub error_message: Option<String>,
    pub prefix_armed: bool,
    pub bar_rows: u16,
    pub rows: u16,
    pub cols: u16,
}

impl App {
    pub fn new(cwd: PathBuf, rows: u16, cols: u16) -> Result<Self> {
        let bar_rows = focus_bar_rows("");
        let term_rows = rows.saturating_sub(bar_rows);
        let session = Session::spawn(cwd, term_rows, cols)?;

        Ok(Self {
            state: AppState::Focus(0),
            sessions: vec![session],
            selected: 0,
            input_mode: InputMode::Normal,
            should_quit: false,
            error_message: None,
            prefix_armed: false,
            bar_rows,
            rows,
            cols,
        })
    }

    pub fn draw_overview(&self, frame: &mut Frame) {
        ui::overview::draw(frame, self);
    }

    pub fn handle_overview_key(&mut self, key: KeyEvent) -> Result<()> {
        match &self.input_mode {
            InputMode::NewSession(_) => self.handle_new_session_input(key),
            InputMode::Normal => self.handle_overview_normal(key),
        }
    }

    fn handle_overview_normal(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('n') => {
                self.input_mode = InputMode::NewSession(String::new());
                self.error_message = None;
            }
            KeyCode::Enter => {
                if !self.sessions.is_empty() {
                    self.state = AppState::Focus(self.selected);
                }
            }
            KeyCode::Char('d') => {
                if !self.sessions.is_empty() {
                    self.sessions.remove(self.selected);
                    if self.selected > 0 && self.selected >= self.sessions.len() {
                        self.selected = self.sessions.len().saturating_sub(1);
                    }
                }
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection_up(),
            KeyCode::Char('h') | KeyCode::Left => self.move_selection_left(),
            KeyCode::Char('l') | KeyCode::Right => self.move_selection_right(),
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.sessions.len() {
                    self.selected = idx;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_new_session_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.error_message = None;
            }
            KeyCode::Enter => {
                if let InputMode::NewSession(ref path_str) = self.input_mode {
                    let expanded = shellexpand::tilde(path_str).to_string();
                    let path = PathBuf::from(&expanded);
                    if path.is_dir() {
                        let term_rows = self.rows.saturating_sub(self.bar_rows);
                        match Session::spawn(path, term_rows, self.cols) {
                            Ok(session) => {
                                self.sessions.push(session);
                                self.selected = self.sessions.len() - 1;
                                self.input_mode = InputMode::Normal;
                                self.error_message = None;
                            }
                            Err(e) => {
                                self.error_message =
                                    Some(format!("Failed to spawn session: {e}"));
                            }
                        }
                    } else {
                        self.error_message =
                            Some(format!("Not a valid directory: {expanded}"));
                    }
                }
            }
            KeyCode::Char(c) => {
                if let InputMode::NewSession(ref mut s) = self.input_mode {
                    s.push(c);
                }
            }
            KeyCode::Backspace => {
                if let InputMode::NewSession(ref mut s) = self.input_mode {
                    s.pop();
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn process_all_sessions(&mut self) {
        for session in &mut self.sessions {
            session.process_pty_output();
        }
    }

    fn grid_cols(&self) -> usize {
        let n = self.sessions.len();
        match n {
            0 | 1 => 1,
            2 => 2,
            3 | 4 => 2,
            5 | 6 => 3,
            _ => 3,
        }
    }

    fn move_selection_down(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let cols = self.grid_cols();
        let next = self.selected + cols;
        if next < self.sessions.len() {
            self.selected = next;
        }
    }

    fn move_selection_up(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let cols = self.grid_cols();
        if self.selected >= cols {
            self.selected -= cols;
        }
    }

    fn move_selection_right(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let next = self.selected + 1;
        if next < self.sessions.len() {
            self.selected = next;
        }
    }

    fn move_selection_left(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
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

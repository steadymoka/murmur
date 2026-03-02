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

    pub fn poll_event(timeout: std::time::Duration) -> Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }
}

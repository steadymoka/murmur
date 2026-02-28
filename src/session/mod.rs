use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::Result;
use portable_pty::{CommandBuilder, MasterPty, PtySize};

#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Running,
    Exited(u32),
}

pub struct Session {
    pub name: String,
    pub cwd: PathBuf,
    pub pinned_prompt: String,
    pub input_buffer: String,
    pub status: SessionStatus,
    pub was_alternate_screen: bool,
    parser: vt100::Parser,
    pty_rx: mpsc::Receiver<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl Session {
    pub fn spawn(cwd: PathBuf, rows: u16, cols: u16) -> Result<Self> {
        let name = cwd
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| cwd.to_string_lossy().to_string());

        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(&cwd);

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let parser = vt100::Parser::new(rows, cols, 0);

        Ok(Session {
            name,
            cwd,
            pinned_prompt: String::new(),
            input_buffer: String::new(),
            status: SessionStatus::Running,
            was_alternate_screen: false,
            parser,
            pty_rx: rx,
            master: pair.master,
            writer,
            _child: child,
        })
    }

    /// Drain raw byte chunks from the PTY channel without parsing.
    /// Used by Focus mode to forward raw output to stdout.
    pub fn drain_raw_chunks(&mut self) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        while let Ok(bytes) = self.pty_rx.try_recv() {
            chunks.push(bytes);
        }
        chunks
    }

    /// Feed raw bytes into the vt100 parser only (no scrollback tracking).
    /// Used after drain_raw_chunks to keep parser state in sync.
    pub fn feed_parser(&mut self, data: &[u8]) {
        let was_alt = self.parser.screen().alternate_screen();
        self.parser.process(data);
        let is_alt = self.parser.screen().alternate_screen();
        if was_alt != is_alt {
            self.was_alternate_screen = is_alt;
        }
    }

    /// Process PTY output for Overview mode (drain + parse, no scrollback).
    pub fn process_pty_output(&mut self) {
        let chunks = self.drain_raw_chunks();
        for chunk in &chunks {
            self.feed_parser(chunk);
        }
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        self.parser.screen_mut().set_size(rows, cols);
        Ok(())
    }

    pub fn track_input(&mut self, c: char) {
        match c {
            '\r' => {
                let trimmed = self.input_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    self.pinned_prompt = trimmed;
                }
                self.input_buffer.clear();
            }
            '\n' => {
                self.input_buffer.push('\n');
            }
            '\x03' => {
                self.input_buffer.clear();
            }
            '\x7f' | '\x08' => {
                self.input_buffer.pop();
            }
            '\x15' => {
                self.input_buffer.clear();
            }
            c if !c.is_control() => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }
}

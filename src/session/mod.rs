use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use portable_pty::{CommandBuilder, MasterPty, PtySize};

#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Running,
    Exited(u32),
}

struct TitleTracker {
    title: Arc<Mutex<String>>,
}

impl vt100::Callbacks for TitleTracker {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        if let Ok(mut t) = self.title.lock() {
            *t = String::from_utf8_lossy(title).to_string();
        }
    }
}

pub struct Session {
    pub name: String,
    pub cwd: PathBuf,
    pub pinned_prompt: String,
    pub input_buffer: String,
    pub status: SessionStatus,
    pub was_alternate_screen: bool,
    window_title: Arc<Mutex<String>>,
    parser: vt100::Parser<TitleTracker>,
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

        let title_arc = Arc::new(Mutex::new(String::new()));
        let tracker = TitleTracker {
            title: Arc::clone(&title_arc),
        };
        let parser = vt100::Parser::new_with_callbacks(rows, cols, 0, tracker);

        Ok(Session {
            name,
            cwd,
            pinned_prompt: String::new(),
            input_buffer: String::new(),
            status: SessionStatus::Running,
            was_alternate_screen: false,
            window_title: title_arc,
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

    pub fn window_title(&self) -> String {
        self.window_title
            .lock()
            .map(|t| t.clone())
            .unwrap_or_default()
    }

    pub fn is_ai_tool(&self) -> bool {
        let title = self.window_title();
        let lower = title.to_ascii_lowercase();
        lower.contains("claude") || lower.contains("codex")
    }

    pub fn ai_tool_name(&self) -> &'static str {
        let title = self.window_title();
        let lower = title.to_ascii_lowercase();
        if lower.contains("claude") {
            "Claude"
        } else if lower.contains("codex") {
            "Codex"
        } else {
            "AI"
        }
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

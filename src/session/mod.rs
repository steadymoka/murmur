mod history;
mod pin;
mod proc_name;
mod prompt;
mod selection;

pub use pin::PinHistory;

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use portable_pty::{CommandBuilder, MasterPty, PtySize};

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
    pub pins: PinHistory,
    pin_pending_ts: Option<u128>,
    project_path: String,
    window_title: Arc<Mutex<String>>,
    parser: vt100::Parser<TitleTracker>,
    pty_rx: mpsc::Receiver<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl Session {
    pub fn spawn(cwd: PathBuf, rows: u16, cols: u16) -> Result<Self> {
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
            pins: PinHistory::new(),
            pin_pending_ts: None,
            project_path: cwd.to_string_lossy().to_string(),
            window_title: title_arc,
            parser,
            pty_rx: rx,
            master: pair.master,
            writer,
            _child: child,
        })
    }

    pub fn drain_raw_chunks(&mut self) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        while let Ok(bytes) = self.pty_rx.try_recv() {
            chunks.push(bytes);
        }
        chunks
    }

    pub fn feed_parser(&mut self, data: &[u8]) {
        self.parser.process(data);
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
        let matched_proc = self
            .master
            .process_group_leader()
            .and_then(proc_name::from_pid)
            .is_some_and(|name| is_ai_tool_name(&name));
        if matched_proc {
            return true;
        }
        is_ai_tool_title(&self.window_title())
    }

    /// Extract prompt from the screen and save as PIN.
    /// Called when Enter is pressed inside an AI tool.
    pub fn record_pin(&mut self) {
        if let Some(text) = prompt::extract_input_area(self.parser.screen()) {
            self.pins.push(text);
        } else if let Some(selected) = selection::extract_selected_option(self.parser.screen()) {
            self.pins.push(selected);
        }
        self.pin_pending_ts = Some(now_ms());
    }

    /// Try to update the last PIN from `~/.claude/history.jsonl`.
    /// Called after PTY output is processed. If a newer history entry exists
    /// for this project, it replaces the screen-based capture with the exact
    /// submitted prompt (including slash command expansion).
    pub fn try_update_pin(&mut self) {
        let Some(ts) = self.pin_pending_ts.take() else {
            return;
        };
        if let Some(prompt) = history::read_latest_prompt(ts, &self.project_path) {
            let current = self.pins.current().to_string();
            if !current.is_empty() && prompt != current {
                self.pins.update_last(prompt);
            }
        }
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

const AI_TOOL_KEYWORDS: &[&str] = &["claude", "codex"];

fn is_ai_tool_name(name: &str) -> bool {
    AI_TOOL_KEYWORDS
        .iter()
        .any(|kw| name.eq_ignore_ascii_case(kw))
}

fn is_ai_tool_title(title: &str) -> bool {
    AI_TOOL_KEYWORDS.iter().any(|kw| {
        title
            .as_bytes()
            .windows(kw.len())
            .any(|w| w.eq_ignore_ascii_case(kw.as_bytes()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_ai_tool_title ─────────────────────────────────────────────

    #[test]
    fn ai_tool_claude() {
        assert!(is_ai_tool_title("Claude Code"));
    }

    #[test]
    fn ai_tool_codex() {
        assert!(is_ai_tool_title("Codex"));
    }

    #[test]
    fn ai_tool_other() {
        assert!(!is_ai_tool_title("vim"));
        assert!(!is_ai_tool_title(""));
    }

    #[test]
    fn ai_tool_case_insensitive() {
        assert!(is_ai_tool_title("CLAUDE"));
        assert!(is_ai_tool_title("cLaUdE"));
        assert!(is_ai_tool_title("CODEX"));
    }

    #[test]
    fn ai_tool_title_real_patterns() {
        assert!(is_ai_tool_title("~/project — Claude Code"));
        assert!(is_ai_tool_title("murmur (main) — Claude Code"));
        assert!(is_ai_tool_title("codex — ~/my-project"));
    }

    #[test]
    fn ai_tool_title_keyword_in_path() {
        assert!(is_ai_tool_title("vim ~/claude-project/main.rs"));
        assert!(is_ai_tool_title("/home/user/codex/file.txt"));
    }

    #[test]
    fn ai_tool_title_shorter_than_keyword() {
        assert!(!is_ai_tool_title("zsh"));
        assert!(!is_ai_tool_title("vi"));
        assert!(!is_ai_tool_title("a"));
    }

    #[test]
    fn ai_tool_title_unrelated() {
        assert!(!is_ai_tool_title("node server.js"));
        assert!(!is_ai_tool_title("npm run dev"));
        assert!(!is_ai_tool_title("cargo test"));
        assert!(!is_ai_tool_title("~/Workspace/my-project"));
    }

    // ── is_ai_tool_name ──────────────────────────────────────────────

    #[test]
    fn ai_tool_name_exact() {
        assert!(is_ai_tool_name("claude"));
        assert!(is_ai_tool_name("Claude"));
        assert!(is_ai_tool_name("codex"));
        assert!(is_ai_tool_name("CODEX"));
    }

    #[test]
    fn ai_tool_name_rejects_other() {
        assert!(!is_ai_tool_name("node"));
        assert!(!is_ai_tool_name("zsh"));
        assert!(!is_ai_tool_name(""));
    }

    #[test]
    fn ai_tool_name_rejects_partial() {
        assert!(!is_ai_tool_name("claude-code"));
        assert!(!is_ai_tool_name("codex-cli"));
        assert!(!is_ai_tool_name("my-claude"));
    }
}

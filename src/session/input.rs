pub struct InputTracker {
    buffer: String,
}

impl InputTracker {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Track a character of user input.
    /// Returns `Some(command)` when Enter is pressed and the buffer contains non-empty text.
    pub fn track(&mut self, c: char) -> Option<String> {
        match c {
            '\r' => {
                let trimmed = self.buffer.trim().to_string();
                self.buffer.clear();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }
            '\n' => {
                self.buffer.push('\n');
                None
            }
            '\x03' => {
                self.buffer.clear();
                None
            }
            '\x7f' | '\x08' => {
                self.buffer.pop();
                None
            }
            '\x15' => {
                self.buffer.clear();
                None
            }
            c if !c.is_control() => {
                self.buffer.push(c);
                None
            }
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn buffer(&self) -> &str {
        &self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_printable() {
        let mut t = InputTracker::new();
        assert_eq!(t.track('h'), None);
        assert_eq!(t.track('i'), None);
        assert_eq!(t.buffer(), "hi");
    }

    #[test]
    fn track_enter_returns_command() {
        let mut t = InputTracker::new();
        t.track('h');
        t.track('e');
        t.track('l');
        t.track('l');
        t.track('o');
        assert_eq!(t.track('\r'), Some("hello".into()));
        assert_eq!(t.buffer(), "");
    }

    #[test]
    fn track_enter_empty_returns_none() {
        let mut t = InputTracker::new();
        assert_eq!(t.track('\r'), None);
    }

    #[test]
    fn track_enter_whitespace_returns_none() {
        let mut t = InputTracker::new();
        t.track(' ');
        t.track(' ');
        assert_eq!(t.track('\r'), None);
    }

    #[test]
    fn track_backspace() {
        let mut t = InputTracker::new();
        t.track('a');
        t.track('b');
        t.track('\x7f');
        assert_eq!(t.buffer(), "a");
    }

    #[test]
    fn track_backspace_empty() {
        let mut t = InputTracker::new();
        t.track('\x7f');
        assert_eq!(t.buffer(), "");
    }

    #[test]
    fn track_ctrl_c_clears() {
        let mut t = InputTracker::new();
        t.track('h');
        t.track('e');
        t.track('\x03');
        assert_eq!(t.buffer(), "");
    }

    #[test]
    fn track_ctrl_u_clears() {
        let mut t = InputTracker::new();
        t.track('h');
        t.track('e');
        t.track('\x15');
        assert_eq!(t.buffer(), "");
    }

    #[test]
    fn track_newline_appends() {
        let mut t = InputTracker::new();
        t.track('a');
        t.track('\n');
        t.track('b');
        assert_eq!(t.buffer(), "a\nb");
    }

    #[test]
    fn track_control_chars_ignored() {
        let mut t = InputTracker::new();
        t.track('a');
        t.track('\x01'); // Ctrl+A
        t.track('\x02'); // Ctrl+B
        assert_eq!(t.buffer(), "a");
    }

    #[test]
    fn track_multiline_enter() {
        let mut t = InputTracker::new();
        t.track('l');
        t.track('i');
        t.track('n');
        t.track('e');
        t.track('1');
        t.track('\n');
        t.track('l');
        t.track('i');
        t.track('n');
        t.track('e');
        t.track('2');
        assert_eq!(t.track('\r'), Some("line1\nline2".into()));
    }
}

const PIN_HISTORY_MAX: usize = 50;

pub struct PinHistory {
    entries: Vec<String>,
    index: Option<usize>,
}

impl PinHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: None,
        }
    }

    pub fn push(&mut self, entry: String) {
        self.entries.push(entry);
        if self.entries.len() > PIN_HISTORY_MAX {
            self.entries.remove(0);
        }
        self.index = None;
    }

    pub fn current(&self) -> &str {
        match self.index {
            Some(i) => self.entries.get(i).map(|s| s.as_str()).unwrap_or(""),
            None => self.entries.last().map(|s| s.as_str()).unwrap_or(""),
        }
    }

    /// Returns `Some((1-based position, total))` when navigating history, `None` when at latest.
    pub fn position(&self) -> Option<(usize, usize)> {
        self.index.map(|i| (i + 1, self.entries.len()))
    }

    pub fn prev(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        match self.index {
            None => {
                if self.entries.len() >= 2 {
                    self.index = Some(self.entries.len() - 2);
                    true
                } else {
                    false
                }
            }
            Some(0) => false,
            Some(i) => {
                self.index = Some(i - 1);
                true
            }
        }
    }

    pub fn next(&mut self) -> bool {
        match self.index {
            None => false,
            Some(i) if i + 1 >= self.entries.len() => {
                self.index = None;
                true
            }
            Some(i) => {
                self.index = Some(i + 1);
                true
            }
        }
    }

    pub fn delete(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let remove_idx = self.index.unwrap_or(self.entries.len() - 1);
        self.entries.remove(remove_idx);
        if self.entries.is_empty() {
            self.index = None;
        } else if let Some(i) = self.index {
            if i >= self.entries.len() {
                self.index = None;
            }
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let pins = PinHistory::new();
        assert!(pins.is_empty());
        assert_eq!(pins.current(), "");
        assert_eq!(pins.position(), None);
    }

    #[test]
    fn push_single() {
        let mut pins = PinHistory::new();
        pins.push("hello".into());
        assert_eq!(pins.current(), "hello");
        assert!(!pins.is_empty());
    }

    #[test]
    fn push_preserves_latest() {
        let mut pins = PinHistory::new();
        pins.push("first".into());
        pins.push("second".into());
        assert_eq!(pins.current(), "second");
    }

    #[test]
    fn push_resets_index() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        pins.push("b".into());
        pins.prev();
        assert_eq!(pins.current(), "a");
        pins.push("c".into());
        assert_eq!(pins.current(), "c");
        assert_eq!(pins.position(), None);
    }

    #[test]
    fn prev_from_latest() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        pins.push("b".into());
        assert!(pins.prev());
        assert_eq!(pins.current(), "a");
        assert_eq!(pins.position(), Some((1, 2)));
    }

    #[test]
    fn prev_at_beginning() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        pins.push("b".into());
        pins.prev();
        assert!(!pins.prev());
        assert_eq!(pins.current(), "a");
    }

    #[test]
    fn prev_empty() {
        let mut pins = PinHistory::new();
        assert!(!pins.prev());
    }

    #[test]
    fn prev_single_entry() {
        let mut pins = PinHistory::new();
        pins.push("only".into());
        assert!(!pins.prev());
        assert_eq!(pins.current(), "only");
    }

    #[test]
    fn next_from_history() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        pins.push("b".into());
        pins.push("c".into());
        pins.prev();
        pins.prev();
        assert_eq!(pins.current(), "a");
        assert!(pins.next());
        assert_eq!(pins.current(), "b");
    }

    #[test]
    fn next_returns_to_latest() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        pins.push("b".into());
        pins.prev();
        assert_eq!(pins.current(), "a");
        assert!(pins.next());
        // At last index but still in navigation mode
        assert_eq!(pins.current(), "b");
        assert_eq!(pins.position(), Some((2, 2)));
        // One more next exits navigation
        assert!(pins.next());
        assert_eq!(pins.current(), "b");
        assert_eq!(pins.position(), None);
    }

    #[test]
    fn next_at_latest() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        assert!(!pins.next());
    }

    #[test]
    fn delete_at_latest() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        pins.push("b".into());
        pins.delete();
        assert_eq!(pins.current(), "a");
    }

    #[test]
    fn delete_at_index() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        pins.push("b".into());
        pins.push("c".into());
        pins.prev();
        assert_eq!(pins.current(), "b");
        pins.delete();
        // After removing "b" from ["a","b","c"], entries = ["a","c"], index stays at 1
        assert_eq!(pins.current(), "c");
        assert_eq!(pins.position(), Some((2, 2)));
    }

    #[test]
    fn delete_last_entry() {
        let mut pins = PinHistory::new();
        pins.push("only".into());
        pins.delete();
        assert!(pins.is_empty());
        assert_eq!(pins.current(), "");
        assert_eq!(pins.position(), None);
    }

    #[test]
    fn position_when_navigating() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        pins.push("b".into());
        pins.push("c".into());
        pins.prev();
        assert_eq!(pins.position(), Some((2, 3)));
        pins.prev();
        assert_eq!(pins.position(), Some((1, 3)));
    }

    #[test]
    fn position_at_latest() {
        let mut pins = PinHistory::new();
        pins.push("a".into());
        assert_eq!(pins.position(), None);
    }

    #[test]
    fn push_enforces_max() {
        let mut pins = PinHistory::new();
        for i in 0..=PIN_HISTORY_MAX {
            pins.push(format!("entry-{}", i));
        }
        assert_eq!(pins.entries.len(), PIN_HISTORY_MAX);
        assert_eq!(pins.current(), &format!("entry-{}", PIN_HISTORY_MAX));
        assert_eq!(pins.entries[0], "entry-1");
    }
}

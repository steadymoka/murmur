use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn ctrl_byte(c: char) -> Option<u8> {
    let b = c.to_ascii_lowercase() as u8;
    b.is_ascii_lowercase().then(|| b - b'a' + 1)
}

/// Convert a crossterm KeyEvent to raw bytes suitable for PTY input.
pub fn key_event_to_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    let mut bytes = match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                vec![ctrl_byte(c)?]
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press)
    }

    #[test]
    fn bytes_char() {
        let key = make_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(key_event_to_bytes(&key), Some(vec![b'a']));
    }

    #[test]
    fn bytes_ctrl_c() {
        let key = make_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_event_to_bytes(&key), Some(vec![3]));
    }

    #[test]
    fn bytes_ctrl_a() {
        let key = make_key(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(key_event_to_bytes(&key), Some(vec![1]));
    }

    #[test]
    fn bytes_alt_a() {
        let key = make_key(KeyCode::Char('a'), KeyModifiers::ALT);
        assert_eq!(key_event_to_bytes(&key), Some(vec![0x1b, b'a']));
    }

    #[test]
    fn bytes_enter() {
        let key = make_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(key_event_to_bytes(&key), Some(vec![b'\r']));
    }

    #[test]
    fn bytes_special_keys() {
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::Up, KeyModifiers::NONE)),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::Down, KeyModifiers::NONE)),
            Some(b"\x1b[B".to_vec())
        );
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::Right, KeyModifiers::NONE)),
            Some(b"\x1b[C".to_vec())
        );
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::Left, KeyModifiers::NONE)),
            Some(b"\x1b[D".to_vec())
        );
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::Home, KeyModifiers::NONE)),
            Some(b"\x1b[H".to_vec())
        );
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::End, KeyModifiers::NONE)),
            Some(b"\x1b[F".to_vec())
        );
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::Delete, KeyModifiers::NONE)),
            Some(b"\x1b[3~".to_vec())
        );
    }

    #[test]
    fn bytes_f_keys() {
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::F(1), KeyModifiers::NONE)),
            Some(b"\x1bOP".to_vec())
        );
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::F(5), KeyModifiers::NONE)),
            Some(b"\x1b[15~".to_vec())
        );
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::F(12), KeyModifiers::NONE)),
            Some(b"\x1b[24~".to_vec())
        );
    }

    #[test]
    fn bytes_utf8() {
        let key = make_key(KeyCode::Char('\u{FF21}'), KeyModifiers::NONE); // Fullwidth A
        let bytes = key_event_to_bytes(&key).unwrap();
        assert_eq!(bytes, "\u{FF21}".as_bytes());
    }

    #[test]
    fn bytes_none_cases() {
        assert_eq!(
            key_event_to_bytes(&make_key(KeyCode::CapsLock, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn bytes_backspace() {
        let key = make_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(key_event_to_bytes(&key), Some(vec![0x7f]));
    }

    #[test]
    fn bytes_tab() {
        let key = make_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(key_event_to_bytes(&key), Some(vec![b'\t']));
    }

    #[test]
    fn bytes_esc() {
        let key = make_key(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(key_event_to_bytes(&key), Some(vec![0x1b]));
    }
}

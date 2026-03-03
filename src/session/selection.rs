const SELECTION_MARKERS: &[char] = &['❯', '▸', '►', '❱'];

/// Extract the selected option text from a vt100 screen by scanning for selection markers.
///
/// Claude Code renders selection UIs with a marker (e.g. `❯`) before the highlighted option.
/// This scans rows bottom-to-top and returns the text after the first marker found.
pub fn extract_selected_option(screen: &vt100::Screen) -> Option<String> {
    let (_rows, cols) = screen.size();
    let row_texts: Vec<String> = screen.rows(0, cols).collect();

    for text in row_texts.iter().rev() {
        if let Some(option) = extract_marker_text(text) {
            return Some(option);
        }
    }
    None
}

fn extract_marker_text(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let first = trimmed.chars().next()?;
    if SELECTION_MARKERS.contains(&first) {
        let rest = trimmed[first.len_utf8()..].trim();
        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_arrow() {
        assert_eq!(extract_marker_text("❯ Option A"), Some("Option A".into()));
    }

    #[test]
    fn marker_triangle() {
        assert_eq!(extract_marker_text("▸ Option B"), Some("Option B".into()));
    }

    #[test]
    fn marker_right_pointer() {
        assert_eq!(extract_marker_text("► Yes"), Some("Yes".into()));
    }

    #[test]
    fn marker_heavy_bracket() {
        assert_eq!(extract_marker_text("❱ No"), Some("No".into()));
    }

    #[test]
    fn marker_with_leading_spaces() {
        assert_eq!(extract_marker_text("  ❯ Indented"), Some("Indented".into()));
    }

    #[test]
    fn no_marker() {
        assert_eq!(extract_marker_text("  Option C"), None);
        assert_eq!(extract_marker_text("plain text"), None);
        assert_eq!(extract_marker_text(""), None);
    }

    #[test]
    fn ascii_greater_than_not_marker() {
        assert_eq!(extract_marker_text("> not a marker"), None);
    }

    #[test]
    fn marker_empty_text() {
        assert_eq!(extract_marker_text("❯   "), None);
        assert_eq!(extract_marker_text("❯"), None);
    }

    #[test]
    fn extract_from_screen_simple() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"? Which option?\r\n  Option A\r\n\xe2\x9d\xaf Option B\r\n  Option C");
        let result = extract_selected_option(parser.screen());
        assert_eq!(result, Some("Option B".into()));
    }

    #[test]
    fn extract_bottom_up_priority() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // Two markers: bottom one should win
        parser
            .process(b"\xe2\x9d\xaf Old selection\r\nsome text\r\n\xe2\x9d\xaf Current selection");
        let result = extract_selected_option(parser.screen());
        assert_eq!(result, Some("Current selection".into()));
    }

    #[test]
    fn extract_no_marker_on_screen() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Just some regular text\r\nNo markers here");
        let result = extract_selected_option(parser.screen());
        assert_eq!(result, None);
    }

    #[test]
    fn extract_empty_screen() {
        let parser = vt100::Parser::new(24, 80, 0);
        let result = extract_selected_option(parser.screen());
        assert_eq!(result, None);
    }
}

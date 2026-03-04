const SELECTION_MARKERS: &[char] = &['❯', '▸', '►', '❱'];

/// Returns true if the selected text looks like a tool permission prompt
/// rather than an AskUserQuestion selection.
///
/// Permission prompts are Yes/No variants (e.g. "Yes", "No, keep planning",
/// "Yes, auto-accept edits"), optionally with a number prefix ("1. Yes").
pub fn is_permission_prompt(text: &str) -> bool {
    let core = strip_number_prefix(text);
    core.eq_ignore_ascii_case("yes")
        || core.eq_ignore_ascii_case("no")
        || starts_with_ignore_ascii_case(core, "yes,")
        || starts_with_ignore_ascii_case(core, "yes ")
        || starts_with_ignore_ascii_case(core, "no,")
        || starts_with_ignore_ascii_case(core, "no ")
}

fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.as_bytes()
        .get(..prefix.len())
        .is_some_and(|b| b.eq_ignore_ascii_case(prefix.as_bytes()))
}

/// Strip optional "N. " number prefix: "1. Yes" → "Yes", "hello" → "hello".
fn strip_number_prefix(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && text[i..].starts_with(". ") {
        &text[i + 2..]
    } else {
        text
    }
}

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

    // ── is_permission_prompt ────────────────────────────────────────

    #[test]
    fn permission_basic_yes_no() {
        assert!(is_permission_prompt("Yes"));
        assert!(is_permission_prompt("No"));
        assert!(is_permission_prompt("yes"));
        assert!(is_permission_prompt("YES"));
        assert!(is_permission_prompt("no"));
        assert!(is_permission_prompt("NO"));
    }

    #[test]
    fn permission_yes_with_comma_qualifier() {
        assert!(is_permission_prompt("Yes, auto-accept edits"));
        assert!(is_permission_prompt("Yes, and don't ask again for this session"));
        assert!(is_permission_prompt("Yes, and don't ask again for `npm test`"));
        assert!(is_permission_prompt("Yes, and don't ask again for npm:*"));
        assert!(is_permission_prompt("Yes, allow all edits during this session"));
        assert!(is_permission_prompt("Yes, I trust this folder"));
        assert!(is_permission_prompt(
            "Yes, clear context (50% used) and auto-accept edits (shift+tab)"
        ));
    }

    #[test]
    fn permission_yes_with_space_qualifier() {
        assert!(is_permission_prompt("Yes allow all edits during this session"));
        assert!(is_permission_prompt("Yes manually approve edits"));
    }

    #[test]
    fn permission_no_with_qualifier() {
        assert!(is_permission_prompt("No, keep planning"));
        assert!(is_permission_prompt("No, exit Claude Code"));
    }

    #[test]
    fn permission_numbered() {
        assert!(is_permission_prompt("1. Yes"));
        assert!(is_permission_prompt("2. Yes, auto-accept edits"));
        assert!(is_permission_prompt("3. No"));
        assert!(is_permission_prompt("1. No, keep planning"));
        assert!(is_permission_prompt(
            "42. Yes, and don't ask again for this session"
        ));
    }

    #[test]
    fn not_permission_custom_options() {
        assert!(!is_permission_prompt("1. 한식"));
        assert!(!is_permission_prompt("양식"));
        assert!(!is_permission_prompt("Option A"));
        assert!(!is_permission_prompt("파스타, 피자, 버거 등"));
    }

    #[test]
    fn not_permission_system_options() {
        assert!(!is_permission_prompt("Type something."));
        assert!(!is_permission_prompt("Chat about this"));
        assert!(!is_permission_prompt("Skip interview and plan immediately"));
        assert!(!is_permission_prompt("Other"));
    }

    #[test]
    fn not_permission_words_starting_with_no() {
        assert!(!is_permission_prompt("Notify admin"));
        assert!(!is_permission_prompt("None of the above"));
        assert!(!is_permission_prompt("Normal mode"));
    }

    #[test]
    fn not_permission_words_starting_with_yes() {
        assert!(!is_permission_prompt("Yesterday was great"));
    }

    // ── strip_number_prefix ─────────────────────────────────────────

    #[test]
    fn strip_prefix_with_number() {
        assert_eq!(strip_number_prefix("1. Yes"), "Yes");
        assert_eq!(strip_number_prefix("12. Option"), "Option");
    }

    #[test]
    fn strip_prefix_no_number() {
        assert_eq!(strip_number_prefix("Yes"), "Yes");
        assert_eq!(strip_number_prefix("한식"), "한식");
    }

    #[test]
    fn strip_prefix_no_dot_space() {
        assert_eq!(strip_number_prefix("1Yes"), "1Yes");
        assert_eq!(strip_number_prefix("1.Yes"), "1.Yes");
    }
}

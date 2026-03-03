const SEPARATOR_CHAR: char = '─';
const SEPARATOR_MIN_LEN: usize = 10;

/// Prefix used in Claude Code's input area: ❯ followed by non-breaking space (U+00A0).
const INPUT_PREFIX: &str = "❯\u{a0}";

/// Prefix used in Claude Code's conversation history: ❯ followed by regular space.
const CONVERSATION_PREFIX: &str = "❯ ";

fn is_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= SEPARATOR_MIN_LEN && trimmed.chars().all(|c| c == SEPARATOR_CHAR)
}

/// Extract prompt text from Claude Code's input area (between ──── separators).
///
/// The input area uses `❯\u{a0}` (NBSP) prefix, distinguishing it from conversation
/// entries which use `❯ ` (regular space).
pub fn extract_input_area(screen: &vt100::Screen) -> Option<String> {
    let (_rows, cols) = screen.size();
    let lines: Vec<String> = screen.rows(0, cols).collect();

    // Scan bottom-to-top for the last separator pair containing ❯\u{a0}
    let mut lower_sep = None;
    for i in (0..lines.len()).rev() {
        if is_separator(&lines[i]) {
            if lower_sep.is_none() {
                lower_sep = Some(i);
            } else {
                // Found upper separator — extract between them
                let upper = i;
                let lower = lower_sep.unwrap();
                return extract_between(upper, lower, &lines);
            }
        }
    }
    None
}

fn extract_between(upper: usize, lower: usize, lines: &[String]) -> Option<String> {
    let mut result_lines = Vec::new();

    for line in &lines[upper + 1..lower] {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !result_lines.is_empty() {
                result_lines.push(String::new());
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix(INPUT_PREFIX) {
            result_lines.push(rest.to_string());
        } else if !result_lines.is_empty() {
            // Continuation line (typically 2-space indented)
            let content = trimmed.trim_start();
            result_lines.push(content.to_string());
        }
    }

    // Trim trailing empty lines
    while result_lines.last().is_some_and(|l| l.is_empty()) {
        result_lines.pop();
    }

    if result_lines.is_empty() {
        return None;
    }

    let text = result_lines.join("\n").trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extract the latest prompt from conversation history (for deferred update).
///
/// Scans bottom-to-top for `❯ ` (regular space) entries outside separator pairs.
pub fn extract_latest_conversation_prompt(screen: &vt100::Screen) -> Option<String> {
    let (_rows, cols) = screen.size();
    let lines: Vec<String> = screen.rows(0, cols).collect();

    // Track whether we're inside a separator-bounded region (input area)
    let mut inside_separators = false;

    for i in (0..lines.len()).rev() {
        if is_separator(&lines[i]) {
            inside_separators = !inside_separators;
            continue;
        }

        if inside_separators {
            continue;
        }

        let trimmed = lines[i].trim_end();
        if let Some(rest) = trimmed.strip_prefix(CONVERSATION_PREFIX) {
            // Skip if this is actually an input area prefix (NBSP)
            if trimmed.starts_with(INPUT_PREFIX) {
                continue;
            }
            let text = rest.trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_screen(text: &[u8]) -> vt100::Parser {
        let mut parser = vt100::Parser::new(30, 160, 0);
        parser.process(text);
        parser
    }

    #[test]
    fn extract_simple_prompt() {
        let mut input = Vec::new();
        input.extend_from_slice(b"conversation text\r\n");
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice("❯\u{a0}hello world\r\n".as_bytes());
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice(b"status bar");

        let parser = make_screen(&input);
        assert_eq!(
            extract_input_area(parser.screen()),
            Some("hello world".into())
        );
    }

    #[test]
    fn extract_multiline_prompt() {
        let mut input = Vec::new();
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice("❯\u{a0}first line\r\n".as_bytes());
        input.extend_from_slice(b"  second line\r\n");
        input.extend_from_slice("────────────────────".as_bytes());

        let parser = make_screen(&input);
        assert_eq!(
            extract_input_area(parser.screen()),
            Some("first line\nsecond line".into())
        );
    }

    #[test]
    fn extract_no_input_area() {
        let parser = make_screen(b"just some text\r\nno separators here");
        assert_eq!(extract_input_area(parser.screen()), None);
    }

    #[test]
    fn extract_empty_input_area() {
        let mut input = Vec::new();
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice("❯\u{a0}\r\n".as_bytes());
        input.extend_from_slice("────────────────────".as_bytes());

        let parser = make_screen(&input);
        assert_eq!(extract_input_area(parser.screen()), None);
    }

    #[test]
    fn extract_slash_command() {
        let mut input = Vec::new();
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice("❯\u{a0}/us\r\n".as_bytes());
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice(b"  /usage        Show plan usage");

        let parser = make_screen(&input);
        assert_eq!(extract_input_area(parser.screen()), Some("/us".into()));
    }

    #[test]
    fn extract_conversation_prompt() {
        let mut input = Vec::new();
        input.extend_from_slice("❯ /usage\r\n".as_bytes());
        input.extend_from_slice(b"  response text\r\n");
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice("❯\u{a0}new prompt\r\n".as_bytes());
        input.extend_from_slice("────────────────────".as_bytes());

        let parser = make_screen(&input);
        assert_eq!(
            extract_latest_conversation_prompt(parser.screen()),
            Some("/usage".into())
        );
    }

    #[test]
    fn conversation_skips_input_area() {
        let mut input = Vec::new();
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice("❯\u{a0}typing here\r\n".as_bytes());
        input.extend_from_slice("────────────────────".as_bytes());

        let parser = make_screen(&input);
        // Should not pick up the input area entry
        assert_eq!(extract_latest_conversation_prompt(parser.screen()), None);
    }

    #[test]
    fn conversation_finds_latest() {
        let mut input = Vec::new();
        input.extend_from_slice("❯ first prompt\r\n".as_bytes());
        input.extend_from_slice(b"response\r\n");
        input.extend_from_slice("❯ second prompt\r\n".as_bytes());
        input.extend_from_slice(b"response");

        let parser = make_screen(&input);
        assert_eq!(
            extract_latest_conversation_prompt(parser.screen()),
            Some("second prompt".into())
        );
    }

    #[test]
    fn extract_pasted_text_placeholder() {
        let mut input = Vec::new();
        input.extend_from_slice("────────────────────\r\n".as_bytes());
        input.extend_from_slice("❯\u{a0}[Pasted text #1 +20 lines]\r\n".as_bytes());
        input.extend_from_slice("────────────────────".as_bytes());

        let parser = make_screen(&input);
        assert_eq!(
            extract_input_area(parser.screen()),
            Some("[Pasted text #1 +20 lines]".into())
        );
    }

    #[test]
    fn separator_detection() {
        assert!(is_separator("────────────────────"));
        assert!(is_separator("  ────────────────────  "));
        assert!(!is_separator("---"));
        assert!(!is_separator(""));
        assert!(!is_separator("some text ──── more text"));
    }
}

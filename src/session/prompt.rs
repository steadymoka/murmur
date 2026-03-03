const SEPARATOR_CHAR: char = '─';
const SEPARATOR_MIN_LEN: usize = 10;

/// Prefix used in Claude Code's input area: ❯ followed by non-breaking space (U+00A0).
const INPUT_PREFIX: &str = "❯\u{a0}";

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

    // ── Real screen dump regression tests ──────────────────────────────
    // Reconstructed from /tmp/murmur-screen-dump.log captures.

    /// Helper: build a vt100 screen from row strings joined with \r\n.
    fn screen_from_rows(rows: &[&str], term_rows: u16, term_cols: u16) -> vt100::Parser {
        let joined = rows.join("\r\n");
        let mut parser = vt100::Parser::new(term_rows, term_cols, 0);
        parser.process(joined.as_bytes());
        parser
    }

    #[test]
    fn dump1_first_prompt_hi() {
        // Dump 1: first prompt "hi" entered
        let rows = &[
            "➜  murmur git:(main) ✗ claude",
            "",
            " ▐▛███▜▌   Claude Code v2.1.63",
            "▝▜█████▛▘  Opus 4.6 (1M context) · Claude Max",
            "  ▘▘ ▝▝    ~/Workspace/@Murmur/murmur",
            "",
            "────────────────────────────────────────────────────────────────",
            "❯\u{a0}hi",
            "────────────────────────────────────────────────────────────────",
            "  Opus 4.6 (1M context) murmur (main) | ctx 0% | v2.1.63",
            "  ⏸ plan mode on (shift+tab to cycle)",
        ];
        let parser = screen_from_rows(rows, 28, 156);
        assert_eq!(extract_input_area(parser.screen()), Some("hi".into()));
    }

    #[test]
    fn dump3_slash_autocomplete_dropdown() {
        // Dump 3: typed "/us" with autocomplete dropdown visible below
        let rows = &[
            "❯ hi",
            "",
            "⏺ hi! How can I help you today?",
            "",
            "❯ kkkk",
            "",
            "⏺ ㅋㅋㅋㅋ 뭔가 도와드릴 일이 있으면 말씀해주세요!",
            "",
            "────────────────────────────────────────────────────────────────",
            "❯\u{a0}/us",
            "────────────────────────────────────────────────────────────────",
            "  /usage                                      Show plan usage limits",
            "  /statusline                                 Set up status line UI",
        ];
        let parser = screen_from_rows(rows, 28, 156);

        // Input area should capture what's typed
        assert_eq!(extract_input_area(parser.screen()), Some("/us".into()));
    }

    #[test]
    fn dump4_after_slash_expanded() {
        // Dump 4: after /us was submitted, conversation shows /usage
        let rows = &[
            "❯ hi",
            "",
            "⏺ hi! How can I help you today?",
            "",
            "❯ kkkk",
            "",
            "⏺ ㅋㅋㅋㅋ 뭔가 도와드릴 일이 있으면 말씀해주세요!",
            "",
            "❯ /usage",
            "  ⎿  Status dialog dismissed",
            "",
            "────────────────────────────────────────────────────────────────",
            "❯\u{a0}hihi",
            "────────────────────────────────────────────────────────────────",
            "  Opus 4.6 (1M context) murmur (main) | ctx 6% | v2.1.63",
        ];
        let parser = screen_from_rows(rows, 28, 156);

        assert_eq!(extract_input_area(parser.screen()), Some("hihi".into()));
    }

    #[test]
    fn dump5_multiline_prompt() {
        // Dump 5: multiline prompt with continuation line
        let rows = &[
            "❯ hihi",
            "",
            "⏺ hihi! 무엇을 도와드릴까요? 😄",
            "",
            "────────────────────────────────────────────────────────────────",
            "❯\u{a0}선택지 한번 보여줘.",
            "  점메추",
            "────────────────────────────────────────────────────────────────",
            "  Opus 4.6 (1M context) murmur (main) | ctx 6%",
        ];
        let parser = screen_from_rows(rows, 28, 156);

        assert_eq!(
            extract_input_area(parser.screen()),
            Some("선택지 한번 보여줘.\n점메추".into())
        );
    }

    #[test]
    fn dump6_selection_ui() {
        // Dump 6: AskUserQuestion selection UI — no input area, has selection markers
        let rows = &[
            "❯ 선택지 한번 보여줘.",
            "  점메추",
            "────────────────────────────────────────────────────────────────",
            "────────────────────────────────────────────────────────────────",
            " ☐ 점메추",
            "",
            "오늘 점심 뭐 먹을까요?",
            "",
            "  1. 한식",
            "     김치찌개, 된장찌개, 비빔밥 등 따뜻한 한식",
            "  2. 일식",
            "     라멘, 돈카츠, 초밥 등",
            "  3. 중식",
            "     짜장면, 짬뽕, 탕수육 등",
            "❯ 4. 양식",
            "     파스타, 피자, 버거 등",
            "  5. Type something.",
            "────────────────────────────────────────────────────────────────",
            "  6. Chat about this",
            "  7. Skip interview and plan immediately",
            "",
            "Enter to select · ↑/↓ to navigate · Esc to cancel",
        ];
        let parser = screen_from_rows(rows, 27, 156);

        // No ❯\u{a0} between separators — extract_input_area should return None
        assert_eq!(extract_input_area(parser.screen()), None);

        // Selection UI falls back to selection::extract_selected_option
        // (tested separately in selection.rs)
    }

    #[test]
    fn dump9_pasted_text() {
        // Dump 9: pasted text placeholder
        let rows = &[
            "⏺ 파스타 결정!",
            "",
            "────────────────────────────────────────────────────────────────",
            "❯\u{a0}[Pasted text #1 +20 lines]",
            "────────────────────────────────────────────────────────────────",
            "  Opus 4.6 (1M context) murmur (main) | ctx 6%",
        ];
        let parser = screen_from_rows(rows, 28, 156);

        assert_eq!(
            extract_input_area(parser.screen()),
            Some("[Pasted text #1 +20 lines]".into())
        );
    }

    #[test]
    fn input_area_not_confused_with_selection_marker() {
        // Selection UI uses "❯ " (regular space), not "❯\u{a0}" (NBSP)
        // Make sure we don't confuse them
        let rows = &[
            "────────────────────────────────────────────────────────────────",
            "────────────────────────────────────────────────────────────────",
            "❯ Option A",
            "  Option B",
            "────────────────────────────────────────────────────────────────",
        ];
        let parser = screen_from_rows(rows, 10, 80);

        // "❯ " (regular space) between separators should NOT be extracted as input area
        assert_eq!(extract_input_area(parser.screen()), None);
    }

    #[test]
    fn multiple_separator_pairs_picks_last() {
        // Screen has conversation separators AND input area separators
        let rows = &[
            "────────────────────────────────────────────────────────────────",
            "────────────────────────────────────────────────────────────────",
            "  selection content",
            "────────────────────────────────────────────────────────────────",
            "  more stuff",
            "────────────────────────────────────────────────────────────────",
            "❯\u{a0}actual prompt",
            "────────────────────────────────────────────────────────────────",
            "  status bar",
        ];
        let parser = screen_from_rows(rows, 15, 80);

        assert_eq!(
            extract_input_area(parser.screen()),
            Some("actual prompt".into())
        );
    }
}

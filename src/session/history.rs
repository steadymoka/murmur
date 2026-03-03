use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

fn history_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".claude").join("history.jsonl")
}

/// Read the latest matching prompt from `~/.claude/history.jsonl`.
///
/// Returns `Some(display)` if the last entry has a `timestamp` > `after_ms`
/// and a `project` matching `project_path`. Otherwise returns `None`.
pub fn read_latest_prompt(after_ms: u128, project_path: &str) -> Option<String> {
    let line = read_last_line(&history_path())?;
    parse_entry(&line, after_ms, project_path)
}

/// Read the last non-empty line from a file by seeking from the end.
fn read_last_line(path: &PathBuf) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len == 0 {
        return None;
    }

    // Read up to 8KB from the end (a single JSONL entry is typically < 4KB)
    let read_size = file_len.min(8192);
    file.seek(SeekFrom::End(-(read_size as i64))).ok()?;

    let mut buf = vec![0u8; read_size as usize];
    file.read_exact(&mut buf).ok()?;

    let text = String::from_utf8_lossy(&buf);
    text.rsplit('\n')
        .find(|line| !line.trim().is_empty())
        .map(|s| s.to_string())
}

fn parse_entry(line: &str, after_ms: u128, project_path: &str) -> Option<String> {
    let timestamp = extract_json_number(line, "timestamp")?;
    if timestamp <= after_ms {
        return None;
    }

    let project = extract_json_string(line, "project")?;
    if project != project_path {
        return None;
    }

    let display = extract_json_string(line, "display")?;
    let trimmed = display.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Extract a JSON string value for a given key from a single-line JSON object.
/// Handles basic escape sequences: \\, \", \n, \t, \uXXXX.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)? + pattern.len();
    let rest = json[start..].trim_start();

    if !rest.starts_with('"') {
        return None;
    }

    let chars: Vec<char> = rest[1..].chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '"' => return Some(result),
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    break;
                }
                match chars[i] {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '/' => result.push('/'),
                    'u' => {
                        // \uXXXX
                        if i + 4 < chars.len() {
                            let hex: String = chars[i + 1..i + 5].iter().collect();
                            if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                if let Some(c) = char::from_u32(cp) {
                                    result.push(c);
                                }
                            }
                            i += 4;
                        }
                    }
                    other => result.push(other),
                }
            }
            c => result.push(c),
        }
        i += 1;
    }
    None
}

/// Extract a JSON number value for a given key.
fn extract_json_number(json: &str, key: &str) -> Option<u128> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)? + pattern.len();
    let rest = json[start..].trim_start();

    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    rest[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_entry() {
        let line = r#"{"display":"hello world","pastedContents":{},"timestamp":1000,"project":"/home/user/project","sessionId":"abc"}"#;
        assert_eq!(
            parse_entry(line, 999, "/home/user/project"),
            Some("hello world".into())
        );
    }

    #[test]
    fn parse_timestamp_too_old() {
        let line = r#"{"display":"hello","timestamp":1000,"project":"/p"}"#;
        assert_eq!(parse_entry(line, 1000, "/p"), None);
        assert_eq!(parse_entry(line, 1001, "/p"), None);
    }

    #[test]
    fn parse_project_mismatch() {
        let line = r#"{"display":"hello","timestamp":1000,"project":"/other"}"#;
        assert_eq!(parse_entry(line, 999, "/my/project"), None);
    }

    #[test]
    fn parse_slash_command_expanded() {
        let line = r#"{"display":"/usage ","timestamp":2000,"project":"/p"}"#;
        assert_eq!(parse_entry(line, 1999, "/p"), Some("/usage".into()));
    }

    #[test]
    fn parse_unicode_escape() {
        // "display": "\uc548\ub155" → "안녕"
        let line = r#"{"display":"\uc548\ub155","timestamp":2000,"project":"/p"}"#;
        assert_eq!(parse_entry(line, 1999, "/p"), Some("안녕".into()));
    }

    #[test]
    fn parse_escaped_quotes() {
        let line = r#"{"display":"say \"hello\"","timestamp":2000,"project":"/p"}"#;
        assert_eq!(parse_entry(line, 1999, "/p"), Some("say \"hello\"".into()));
    }

    #[test]
    fn parse_empty_display() {
        let line = r#"{"display":"","timestamp":2000,"project":"/p"}"#;
        assert_eq!(parse_entry(line, 1999, "/p"), None);
    }

    #[test]
    fn parse_whitespace_display() {
        let line = r#"{"display":"  ","timestamp":2000,"project":"/p"}"#;
        assert_eq!(parse_entry(line, 1999, "/p"), None);
    }

    #[test]
    fn extract_string_basic() {
        let json = r#"{"display":"hello","other":"world"}"#;
        assert_eq!(extract_json_string(json, "display"), Some("hello".into()));
        assert_eq!(extract_json_string(json, "other"), Some("world".into()));
    }

    #[test]
    fn extract_string_with_escapes() {
        let json = r#"{"display":"line1\nline2\ttab"}"#;
        assert_eq!(
            extract_json_string(json, "display"),
            Some("line1\nline2\ttab".into())
        );
    }

    #[test]
    fn extract_string_missing_key() {
        let json = r#"{"display":"hello"}"#;
        assert_eq!(extract_json_string(json, "missing"), None);
    }

    #[test]
    fn extract_number_basic() {
        let json = r#"{"timestamp":1772503362668,"other":42}"#;
        assert_eq!(extract_json_number(json, "timestamp"), Some(1772503362668));
        assert_eq!(extract_json_number(json, "other"), Some(42));
    }

    #[test]
    fn extract_number_missing() {
        let json = r#"{"timestamp":123}"#;
        assert_eq!(extract_json_number(json, "missing"), None);
    }

    #[test]
    fn read_last_line_from_file() {
        let dir = std::env::temp_dir().join("murmur-test-history");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.jsonl");
        std::fs::write(&path, "first line\nsecond line\nthird line\n").unwrap();
        assert_eq!(read_last_line(&path), Some("third line".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

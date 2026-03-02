use std::sync::mpsc;
use std::thread;

/// Returns a channel that will receive `Some(version_string)` if a newer release exists.
pub fn check_for_update() -> mpsc::Receiver<Option<String>> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let result = fetch_latest_version().and_then(|latest| {
            let current = env!("CARGO_PKG_VERSION");
            if is_newer(&latest, current) {
                Some(latest)
            } else {
                None
            }
        });
        let _ = tx.send(result);
    });

    rx
}

fn fetch_latest_version() -> Option<String> {
    let resp = minreq::get("https://api.github.com/repos/steadymoka/murmur/releases/latest")
        .with_header("User-Agent", "murmur")
        .with_timeout(5)
        .send()
        .ok()?;

    if resp.status_code != 200 {
        return None;
    }

    let body = resp.as_str().ok()?;
    extract_tag_name(body)
}

// Hand-rolled to avoid pulling in serde_json for a single field
fn extract_tag_name(json: &str) -> Option<String> {
    let marker = "\"tag_name\"";
    let pos = json.find(marker)?;
    let after = &json[pos + marker.len()..];

    let colon_pos = after.find(':')?;
    let after_colon = &after[colon_pos + 1..];

    let quote_start = after_colon.find('"')?;
    let value_start = &after_colon[quote_start + 1..];
    let quote_end = value_start.find('"')?;

    let tag = &value_start[..quote_end];
    let version = tag.strip_prefix('v').unwrap_or(tag);
    Some(version.to_string())
}

fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        ))
    };

    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.2.0", "0.1.5"));
        assert!(is_newer("0.1.6", "0.1.5"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.5", "0.1.5"));
        assert!(!is_newer("0.1.4", "0.1.5"));
    }

    #[test]
    fn test_extract_tag_name() {
        let json = r#"{"tag_name": "v0.2.0", "name": "Release 0.2.0"}"#;
        assert_eq!(extract_tag_name(json), Some("0.2.0".to_string()));

        let json = r#"{"tag_name": "0.1.6"}"#;
        assert_eq!(extract_tag_name(json), Some("0.1.6".to_string()));
    }
}

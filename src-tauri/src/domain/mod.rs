//! Domain rules shared by the UI commands and the MCP adapter.

pub mod formula;
pub mod growth;
pub mod properties;
pub mod query;

use chrono::{SecondsFormat, Utc};
use serde_json::Value;

pub const PAGE_KINDS: &[&str] = &["page", "project", "database", "record", "canvas"];
pub const TASK_STATUSES: &[&str] = &["todo", "in_progress", "done"];
pub const TASK_PRIORITIES: &[&str] = &["none", "low", "medium", "high", "urgent"];

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn is_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

/// Collects every user-visible string inside a block's props/content JSON.
pub fn extract_text(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                match (k.as_str(), val) {
                    ("text" | "source" | "caption" | "name" | "originalText", Value::String(s)) => {
                        if !s.is_empty() {
                            if !out.is_empty() && !out.ends_with(' ') {
                                out.push(' ');
                            }
                            out.push_str(s);
                        }
                    }
                    (_, Value::Object(_) | Value::Array(_)) => extract_text(val, out),
                    _ => {}
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| extract_text(i, out)),
        _ => {}
    }
}

/// Finds ids of linked workspace entities (page mentions, embeds, task refs, workspace:// links).
pub fn extract_link_targets(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                match (k.as_str(), val) {
                    ("pageId" | "canvasId" | "databaseId" | "taskId", Value::String(s)) if is_uuid(s) => {
                        push_unique(out, s)
                    }
                    ("href" | "link", Value::String(s)) => {
                        if let Some(id) = parse_workspace_link(s) {
                            push_unique(out, &id)
                        }
                    }
                    (_, Value::Object(_) | Value::Array(_)) => extract_link_targets(val, out),
                    _ => {}
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| extract_link_targets(i, out)),
        _ => {}
    }
}

/// `workspace://page/<uuid>` → `<uuid>`
pub fn parse_workspace_link(s: &str) -> Option<String> {
    let rest = s.strip_prefix("workspace://")?;
    let id = rest.rsplit('/').next()?;
    is_uuid(id).then(|| id.to_string())
}

fn push_unique(out: &mut Vec<String>, s: &str) {
    if !out.iter().any(|x| x == s) {
        out.push(s.to_string());
    }
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

/// Case-insensitive excerpt around the first occurrence of `needle`.
pub fn snippet(haystack: &str, needle: &str, radius: usize) -> String {
    let chars: Vec<char> = haystack.chars().collect();
    let lower: Vec<char> = haystack.to_lowercase().chars().collect();
    let n: Vec<char> = needle.to_lowercase().chars().collect();
    if n.is_empty() || lower.len() != chars.len() {
        return truncate_chars(haystack, radius * 2);
    }
    let pos = lower.windows(n.len()).position(|w| w == n.as_slice());
    match pos {
        Some(p) => {
            let start = p.saturating_sub(radius);
            let end = (p + n.len() + radius).min(chars.len());
            let mut s: String = chars[start..end].iter().collect();
            if start > 0 {
                s.insert(0, '…');
            }
            if end < chars.len() {
                s.push('…');
            }
            s
        }
        None => truncate_chars(haystack, radius * 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_text_and_links() {
        let id = new_id();
        let block = json!({
            "content": [{"type": "text", "text": "สวัสดี"}, {"type": "pageMention", "props": {"pageId": id}}],
            "props": {"source": "graph TD; A-->B"}
        });
        let mut text = String::new();
        extract_text(&block, &mut text);
        assert!(text.contains("สวัสดี") && text.contains("graph TD"));
        let mut links = vec![];
        extract_link_targets(&block, &mut links);
        assert_eq!(links, vec![id]);
    }

    #[test]
    fn snippet_handles_thai() {
        let s = snippet("บันทึกการประชุมทีมออกแบบประจำสัปดาห์", "ออกแบบ", 4);
        assert!(s.contains("ออกแบบ"));
    }
}

//! Markdown → BlockNote block JSON (for MCP page writes). Covers headings, paragraphs, bullet / numbered /
//! check lists (nested by indentation), quotes, fenced code, dividers, and inline bold/italic/code/strike/links.

use serde_json::{json, Value};

fn styled(text: &str, styles: Value) -> Value {
    json!({ "type": "text", "text": text, "styles": styles })
}

/// Inline Markdown → BlockNote inline content.
pub fn inline(src: &str) -> Vec<Value> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = vec![];
    let mut buf = String::new();
    let mut i = 0;
    let flush = |buf: &mut String, out: &mut Vec<Value>| {
        if !buf.is_empty() {
            out.push(styled(buf, json!({})));
            buf.clear();
        }
    };
    let find = |from: usize, pat: &str| -> Option<usize> {
        let p: Vec<char> = pat.chars().collect();
        (from..chars.len()).find(|&j| chars[j..].starts_with(&p))
    };
    while i < chars.len() {
        let rest: String = chars[i..].iter().take(2).collect();
        let (marker, style) = match rest.as_str() {
            "**" => ("**", "bold"),
            "~~" => ("~~", "strike"),
            _ if chars[i] == '`' => ("`", "code"),
            _ if chars[i] == '*' || chars[i] == '_' => (if chars[i] == '*' { "*" } else { "_" }, "italic"),
            _ => ("", ""),
        };
        if !marker.is_empty() {
            let start = i + marker.chars().count();
            if let Some(end) = find(start, marker).filter(|e| *e > start) {
                flush(&mut buf, &mut out);
                let inner: String = chars[start..end].iter().collect();
                out.push(styled(&inner, json!({ style: true })));
                i = end + marker.chars().count();
                continue;
            }
        }
        if chars[i] == '[' {
            if let Some(close) = find(i + 1, "](") {
                if let Some(end) = find(close + 2, ")") {
                    flush(&mut buf, &mut out);
                    let text: String = chars[i + 1..close].iter().collect();
                    let href: String = chars[close + 2..end].iter().collect();
                    out.push(json!({ "type": "link", "href": href, "content": [styled(&text, json!({}))] }));
                    i = end + 1;
                    continue;
                }
            }
        }
        buf.push(chars[i]);
        i += 1;
    }
    flush(&mut buf, &mut out);
    out
}

fn block(btype: &str, props: Value, content: Value) -> Value {
    json!({ "id": crate::domain::new_id(), "type": btype, "props": props, "content": content, "children": [] })
}

pub fn to_blocks(md: &str) -> Vec<Value> {
    let lines: Vec<&str> = md.lines().collect();
    let mut out: Vec<Value> = vec![];
    // Stack of (indent, index path) for nesting list items.
    let mut list_stack: Vec<(usize, Vec<usize>)> = vec![];
    let mut para: Vec<String> = vec![];
    let mut i = 0;

    fn push_at(out: &mut Vec<Value>, path: &[usize], b: Value) -> Vec<usize> {
        if path.is_empty() {
            out.push(b);
            return vec![out.len() - 1];
        }
        let mut node = &mut out[path[0]];
        for idx in &path[1..] {
            node = &mut node["children"][*idx];
        }
        let children = node["children"].as_array_mut().unwrap();
        children.push(b);
        let mut p = path.to_vec();
        p.push(children.len() - 1);
        p
    }

    let flush_para = |para: &mut Vec<String>, out: &mut Vec<Value>| {
        if !para.is_empty() {
            out.push(block("paragraph", json!({}), Value::Array(inline(&para.join(" ")))));
            para.clear();
        }
    };

    while i < lines.len() {
        let raw = lines[i];
        let indent = raw.chars().take_while(|c| *c == ' ' || *c == '\t').map(|c| if c == '\t' { 4 } else { 1 }).sum::<usize>();
        let line = raw.trim();
        if line.starts_with("```") {
            flush_para(&mut para, &mut out);
            list_stack.clear();
            let lang = line.trim_start_matches('`').trim().to_string();
            let mut code = vec![];
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            i += 1;
            let text = code.join("\n");
            if lang == "mermaid" {
                out.push(json!({ "id": crate::domain::new_id(), "type": "mermaid", "props": { "source": text }, "children": [] }));
            } else {
                out.push(block("codeBlock", json!({ "language": if lang.is_empty() { "text".to_string() } else { lang } }), json!([styled(&text, json!({}))])));
            }
            continue;
        }
        if line.is_empty() {
            flush_para(&mut para, &mut out);
            list_stack.clear();
            i += 1;
            continue;
        }
        let list_item: Option<(&str, Value, &str)> = if let Some(r) = line.strip_prefix("- [ ] ").or_else(|| line.strip_prefix("* [ ] ")) {
            Some(("checkListItem", json!({ "checked": false }), r))
        } else if let Some(r) = line.strip_prefix("- [x] ").or_else(|| line.strip_prefix("- [X] ")).or_else(|| line.strip_prefix("* [x] ")) {
            Some(("checkListItem", json!({ "checked": true }), r))
        } else if let Some(r) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")).or_else(|| line.strip_prefix("+ ")) {
            Some(("bulletListItem", json!({}), r))
        } else if let Some(pos) = line.find(". ").filter(|p| *p > 0 && *p <= 3 && line[..*p].chars().all(|c| c.is_ascii_digit())) {
            Some(("numberedListItem", json!({}), &line[pos + 2..]))
        } else {
            None
        };
        if let Some((t, props, text)) = list_item {
            flush_para(&mut para, &mut out);
            while list_stack.last().is_some_and(|(ind, _)| *ind >= indent) {
                list_stack.pop();
            }
            let parent: Vec<usize> = list_stack.last().map(|(_, p)| p.clone()).unwrap_or_default();
            let path = push_at(&mut out, &parent, block(t, props, Value::Array(inline(text))));
            list_stack.push((indent, path));
            i += 1;
            continue;
        }
        list_stack.clear();
        if line == "---" || line == "***" || line == "___" {
            flush_para(&mut para, &mut out);
            out.push(json!({ "id": crate::domain::new_id(), "type": "divider", "props": {}, "children": [] }));
        } else if let Some(h) = ["### ", "## ", "# "].iter().find(|h| line.starts_with(**h)) {
            flush_para(&mut para, &mut out);
            let level = h.trim().len();
            out.push(block("heading", json!({ "level": level }), Value::Array(inline(&line[h.len()..]))));
        } else if let Some(q) = line.strip_prefix("> ").or_else(|| line.strip_prefix(">")) {
            flush_para(&mut para, &mut out);
            out.push(block("quote", json!({}), Value::Array(inline(q))));
        } else {
            para.push(line.to_string());
        }
        i += 1;
    }
    flush_para(&mut para, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_markdown() {
        let b = to_blocks("# หัวข้อ\n\nข้อความ **หนา** และ [ลิงก์](https://x.y)\n\n- a\n  - a1\n- [x] done\n1. one\n> quote\n```rust\nfn x() {}\n```\n---");
        let types: Vec<&str> = b.iter().map(|x| x["type"].as_str().unwrap()).collect();
        assert_eq!(types, vec!["heading", "paragraph", "bulletListItem", "checkListItem", "numberedListItem", "quote", "codeBlock", "divider"]);
        assert_eq!(b[2]["children"][0]["type"], "bulletListItem");
        assert_eq!(b[1]["content"][1]["styles"]["bold"], true);
        assert_eq!(b[1]["content"][3]["type"], "link");
        assert_eq!(b[3]["props"]["checked"], true);
    }
}

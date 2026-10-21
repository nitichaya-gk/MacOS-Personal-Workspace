//! Markdown export of pages (also used by MCP read_page) and file exports of databases.

use super::workspace::Workspace;
use super::{databases, pages};
use crate::error::Result;
use crate::infrastructure::filesystem;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;

pub struct Markdown {
    pub text: String,
    pub warnings: Vec<String>,
    pub assets: Vec<String>,
}

struct Ctx<'a> {
    conn: &'a Connection,
    /// Rewrites wsasset:// urls to `<prefix>/<id>`; None keeps the workspace url.
    asset_prefix: Option<String>,
    warnings: Vec<String>,
    assets: Vec<String>,
}

impl Ctx<'_> {
    fn warn(&mut self, w: &str) {
        if !self.warnings.iter().any(|x| x == w) {
            self.warnings.push(w.to_string());
        }
    }

    fn url(&mut self, url: &str) -> String {
        match url.strip_prefix("wsasset://localhost/") {
            Some(id) => {
                if !self.assets.iter().any(|a| a == id) {
                    self.assets.push(id.to_string());
                }
                match &self.asset_prefix {
                    Some(p) => format!("{p}/{id}"),
                    None => url.to_string(),
                }
            }
            None => url.to_string(),
        }
    }

    fn title_of(&self, id: &str) -> String {
        pages::get_meta(self.conn, id).map(|m| if m.title.is_empty() { "Untitled".into() } else { m.title }).unwrap_or_else(|_| "Missing page".into())
    }
}

fn inline(ctx: &mut Ctx, content: &Value) -> String {
    let Some(items) = content.as_array() else { return String::new() };
    let mut out = String::new();
    for it in items {
        match it.get("type").and_then(|t| t.as_str()) {
            Some("text") => {
                let mut s = it.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
                let st = it.get("styles").cloned().unwrap_or(json!({}));
                if s.trim().is_empty() {
                    out.push_str(&s);
                    continue;
                }
                if st.get("code").and_then(|v| v.as_bool()) == Some(true) {
                    s = format!("`{s}`");
                }
                if st.get("bold").and_then(|v| v.as_bool()) == Some(true) {
                    s = format!("**{s}**");
                }
                if st.get("italic").and_then(|v| v.as_bool()) == Some(true) {
                    s = format!("*{s}*");
                }
                if st.get("strike").and_then(|v| v.as_bool()) == Some(true) {
                    s = format!("~~{s}~~");
                }
                if st.get("underline").and_then(|v| v.as_bool()) == Some(true) {
                    s = format!("<u>{s}</u>");
                }
                out.push_str(&s);
            }
            Some("link") => {
                let text = inline(ctx, it.get("content").unwrap_or(&Value::Null));
                let href = it.get("href").and_then(|h| h.as_str()).unwrap_or("");
                out.push_str(&format!("[{text}]({href})"));
            }
            Some("pageMention") => {
                let id = it.pointer("/props/pageId").and_then(|p| p.as_str()).unwrap_or("");
                out.push_str(&format!("[@{}](workspace://page/{id})", ctx.title_of(id)));
            }
            _ => {}
        }
    }
    out
}

fn table(ctx: &mut Ctx, content: &Value) -> String {
    let rows = content.get("rows").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    let mut lines = vec![];
    for (i, row) in rows.iter().enumerate() {
        let cells: Vec<String> = row
            .get("cells")
            .and_then(|c| c.as_array())
            .map(|cells| {
                cells
                    .iter()
                    .map(|c| {
                        let inner = if c.get("type").and_then(|t| t.as_str()) == Some("tableCell") { c.get("content").cloned().unwrap_or(Value::Null) } else { c.clone() };
                        inline(ctx, &inner).replace('|', "\\|").replace('\n', " ")
                    })
                    .collect()
            })
            .unwrap_or_default();
        lines.push(format!("| {} |", cells.join(" | ")));
        if i == 0 {
            lines.push(format!("|{}", " --- |".repeat(cells.len().max(1))));
        }
    }
    lines.join("\n")
}

fn database_table(ctx: &mut Ctx, db_id: &str) -> String {
    let Ok(info) = databases::get(ctx.conn, db_id) else { return "_(missing database)_".into() };
    let Ok(res) = databases::query(ctx.conn, db_id, &databases::QuerySpec { limit: Some(100), ..Default::default() }) else { return String::new() };
    let props: Vec<_> = info.properties.iter().take(6).collect();
    let mut s = format!("**{}**\n\n| {} |\n|{}\n", info.title, props.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(" | "), " --- |".repeat(props.len()));
    for r in &res.records {
        let cells: Vec<String> = props
            .iter()
            .map(|p| {
                let v = match p.ptype.as_str() {
                    "title" => json!(r.title),
                    "created_time" => json!(r.created_at),
                    "last_edited_time" => json!(r.updated_at),
                    _ => r.values.get(&p.id).cloned().unwrap_or(Value::Null),
                };
                crate::domain::properties::display_value(&p.ptype, &p.config, &v).replace('|', "\\|")
            })
            .collect();
        s.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    if res.total > res.records.len() {
        s.push_str(&format!("\n_…and {} more records_\n", res.total - res.records.len()));
    }
    ctx.warn("Embedded database views are exported as a static table of up to 100 records.");
    s
}

fn blocks(ctx: &mut Ctx, list: &[Value], depth: usize, out: &mut Vec<String>) {
    let indent = "    ".repeat(depth);
    let mut number = 0;
    for b in list {
        let btype = b.get("type").and_then(|t| t.as_str()).unwrap_or("paragraph");
        let props = b.get("props").cloned().unwrap_or(json!({}));
        let content = b.get("content").cloned().unwrap_or(Value::Null);
        let text = inline(ctx, &content);
        number = if btype == "numberedListItem" { number + 1 } else { 0 };
        let prop = |k: &str| props.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let line = match btype {
            "heading" => {
                let level = props.get("level").and_then(|l| l.as_u64()).unwrap_or(1).clamp(1, 6) as usize;
                format!("{}{} {text}", indent, "#".repeat(level))
            }
            "bulletListItem" => format!("{indent}- {text}"),
            "numberedListItem" => format!("{indent}{number}. {text}"),
            "checkListItem" => {
                let checked = props.get("checked").and_then(|c| c.as_bool()) == Some(true);
                format!("{indent}- [{}] {text}", if checked { "x" } else { " " })
            }
            "toggleListItem" => {
                ctx.warn("Toggle blocks are exported as expanded list items.");
                format!("{indent}- {text}")
            }
            "quote" => format!("{indent}> {text}"),
            "callout" => format!("{indent}> {} {text}", if prop("emoji").is_empty() { "💡".into() } else { prop("emoji") }),
            "codeBlock" => {
                let raw: String = content.as_array().map(|a| a.iter().filter_map(|t| t.get("text").and_then(|x| x.as_str())).collect()).unwrap_or_default();
                format!("{indent}```{}\n{raw}\n{indent}```", prop("language"))
            }
            "mermaid" => format!("{indent}```mermaid\n{}\n{indent}```", prop("source")),
            "divider" => format!("{indent}---"),
            "table" => table(ctx, &content),
            "image" => {
                let url = ctx.url(&prop("url"));
                format!("{indent}![{}]({url})", if prop("caption").is_empty() { prop("name") } else { prop("caption") })
            }
            "video" | "audio" | "file" => {
                let url = ctx.url(&prop("url"));
                format!("{indent}[{}]({url})", if prop("name").is_empty() { btype.to_string() } else { prop("name") })
            }
            "pageLink" => format!("{indent}[📄 {}](workspace://page/{})", ctx.title_of(&prop("pageId")), prop("pageId")),
            "canvasEmbed" => {
                ctx.warn("Canvas embeds are exported as links; export the canvas itself as PNG or SVG for an image.");
                format!("{indent}[🖼 {}](workspace://page/{})", ctx.title_of(&prop("canvasId")), prop("canvasId"))
            }
            "databaseView" => database_table(ctx, &prop("databaseId")),
            "taskRef" => match super::tasks::get(ctx.conn, &prop("taskId")) {
                Ok(t) => format!(
                    "{indent}- [{}] {}{}",
                    if t.status == "done" { "x" } else { " " },
                    t.title,
                    t.due_date.map(|d| format!(" (due {d})")).unwrap_or_default()
                ),
                Err(_) => format!("{indent}- [ ] (deleted task)"),
            },
            _ => format!("{indent}{text}"),
        };
        out.push(line);
        if let Some(children) = b.get("children").and_then(|c| c.as_array()).filter(|c| !c.is_empty()) {
            let nested = matches!(btype, "bulletListItem" | "numberedListItem" | "checkListItem" | "toggleListItem");
            if !nested {
                out.push(String::new());
            }
            blocks(ctx, children, depth + 1, out);
        }
        if !matches!(btype, "bulletListItem" | "numberedListItem" | "checkListItem" | "toggleListItem") {
            out.push(String::new());
        }
    }
}

pub fn page_markdown(conn: &Connection, page_id: &str, asset_prefix: Option<String>) -> Result<Markdown> {
    let meta = pages::get_meta(conn, page_id)?;
    let list = pages::load_blocks(conn, page_id)?;
    let mut ctx = Ctx { conn, asset_prefix, warnings: vec![], assets: vec![] };
    let mut out = vec![format!("# {}", if meta.title.is_empty() { "Untitled" } else { &meta.title }), String::new()];
    if meta.kind == "record" {
        if let Ok(r) = databases::get_record(conn, page_id) {
            if let Some(db) = databases::database_of_record(conn, page_id)? {
                for p in databases::properties(conn, &db)? {
                    if p.ptype == "title" {
                        continue;
                    }
                    let v = r.values.get(&p.id).cloned().unwrap_or(Value::Null);
                    let s = crate::domain::properties::display_value(&p.ptype, &p.config, &v);
                    if !s.is_empty() {
                        out.push(format!("- **{}:** {s}", p.name));
                    }
                }
                out.push(String::new());
            }
        }
    }
    if meta.kind == "database" {
        out.push(database_table(&mut ctx, page_id));
    }
    blocks(&mut ctx, &list, 0, &mut out);
    let mut text = out.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    Ok(Markdown { text: text.trim_end().to_string() + "\n", warnings: ctx.warnings, assets: ctx.assets })
}

/// Writes `<dest>.md` and copies used attachments into `<stem>_assets/` next to it.
pub fn export_page_markdown(ws: &Workspace, page_id: &str, dest: &Path) -> Result<Value> {
    let stem = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "page".into());
    let assets_dir_name = format!("{stem}_assets");
    let md = page_markdown(&ws.conn, page_id, Some(assets_dir_name.clone()))?;
    if !md.assets.is_empty() {
        let dir = dest.with_file_name(&assets_dir_name);
        std::fs::create_dir_all(&dir)?;
        for id in &md.assets {
            if let Ok(src) = filesystem::asset_path(&ws.root, id) {
                if src.exists() {
                    std::fs::copy(src, dir.join(id))?;
                }
            }
        }
    }
    filesystem::atomic_write(dest, md.text.as_bytes())?;
    Ok(json!({ "path": dest.to_string_lossy(), "warnings": md.warnings, "assets": md.assets.len() }))
}

pub fn export_database(ws: &Workspace, db_id: &str, format: &str, dest: &Path) -> Result<Value> {
    let bytes = match format {
        "csv" => databases::to_csv(&ws.conn, db_id)?.into_bytes(),
        _ => serde_json::to_vec_pretty(&databases::to_json(&ws.conn, db_id)?)?,
    };
    filesystem::atomic_write(dest, &bytes)?;
    let mut warnings = vec![];
    if format == "csv" {
        warnings.push("CSV keeps values as text; page content inside records is not included.".to_string());
    }
    Ok(json!({ "path": dest.to_string_lossy(), "warnings": warnings }))
}

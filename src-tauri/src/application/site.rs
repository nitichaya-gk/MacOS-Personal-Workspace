//! Static website export (AC-19): only the selected pages are written, internal links between
//! exported pages become relative links, links to anything not exported become plain text,
//! and only attachments used by exported pages are copied.

use super::workspace::Workspace;
use super::{databases, pages};
use crate::error::{invalid, Result};
use crate::infrastructure::filesystem;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

struct Ctx<'a> {
    ws: &'a Workspace,
    exported: &'a HashSet<String>,
    titles: &'a HashMap<String, String>,
    svgs: &'a HashMap<String, String>,
    assets: HashSet<String>,
}

impl Ctx<'_> {
    fn page_link(&self, id: &str, label: &str) -> String {
        if self.exported.contains(id) {
            format!("<a href=\"{id}.html\">{}</a>", esc(label))
        } else {
            esc(label)
        }
    }
    fn asset(&mut self, url: &str) -> Option<String> {
        let id = url.strip_prefix("wsasset://localhost/")?;
        filesystem::check_asset_id(id).ok()?;
        self.assets.insert(id.to_string());
        Some(format!("assets/{id}"))
    }
    fn title(&self, id: &str) -> String {
        self.titles.get(id).cloned().unwrap_or_else(|| pages::get_meta(&self.ws.conn, id).map(|m| m.title).unwrap_or_default())
    }
}

fn inline(ctx: &mut Ctx, content: &Value) -> String {
    let mut out = String::new();
    for it in content.as_array().into_iter().flatten() {
        match it["type"].as_str() {
            Some("text") => {
                let mut s = esc(it["text"].as_str().unwrap_or("")).replace('\n', "<br>");
                let st = &it["styles"];
                for (k, open, close) in [("code", "<code>", "</code>"), ("bold", "<strong>", "</strong>"), ("italic", "<em>", "</em>"), ("strike", "<s>", "</s>"), ("underline", "<u>", "</u>")] {
                    if st[k] == true {
                        s = format!("{open}{s}{close}");
                    }
                }
                out.push_str(&s);
            }
            Some("link") => {
                let text = inline(ctx, &it["content"]);
                let href = it["href"].as_str().unwrap_or("");
                match crate::domain::parse_workspace_link(href) {
                    Some(id) if ctx.exported.contains(&id) => out.push_str(&format!("<a href=\"{id}.html\">{text}</a>")),
                    Some(_) => out.push_str(&text),
                    None if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("mailto:") => {
                        out.push_str(&format!("<a href=\"{}\" rel=\"noopener\">{text}</a>", esc(href)))
                    }
                    None => out.push_str(&text),
                }
            }
            Some("pageMention") => {
                let id = it["props"]["pageId"].as_str().unwrap_or("");
                let t = ctx.title(id);
                out.push_str(&ctx.page_link(id, &format!("@{t}")));
            }
            _ => {}
        }
    }
    out
}

fn table(ctx: &mut Ctx, content: &Value) -> String {
    let mut s = String::from("<table>");
    for (i, row) in content["rows"].as_array().into_iter().flatten().enumerate() {
        s.push_str("<tr>");
        for c in row["cells"].as_array().into_iter().flatten() {
            let inner = if c["type"] == "tableCell" { c["content"].clone() } else { c.clone() };
            let tag = if i == 0 { "th" } else { "td" };
            s.push_str(&format!("<{tag}>{}</{tag}>", inline(ctx, &inner)));
        }
        s.push_str("</tr>");
    }
    s + "</table>"
}

fn database(ctx: &mut Ctx, db_id: &str) -> String {
    if !ctx.exported.contains(db_id) {
        return String::new();
    }
    let Ok(info) = databases::get(&ctx.ws.conn, db_id) else { return String::new() };
    let Ok(res) = databases::query(&ctx.ws.conn, db_id, &databases::QuerySpec { limit: Some(1000), ..Default::default() }) else { return String::new() };
    let mut s = format!("<table class=\"db\"><tr>{}</tr>", info.properties.iter().map(|p| format!("<th>{}</th>", esc(&p.name))).collect::<String>());
    for r in &res.records {
        s.push_str("<tr>");
        for p in &info.properties {
            let cell = if p.ptype == "title" { ctx.page_link(&r.id, &r.title) } else { esc(&databases::record_display(p, r)) };
            s.push_str(&format!("<td>{cell}</td>"));
        }
        s.push_str("</tr>");
    }
    s + "</table>"
}

fn blocks(ctx: &mut Ctx, list: &[Value]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < list.len() {
        let b = &list[i];
        let t = b["type"].as_str().unwrap_or("paragraph");
        // Group consecutive list items into one list element.
        if matches!(t, "bulletListItem" | "numberedListItem" | "checkListItem" | "toggleListItem") {
            let tag = if t == "numberedListItem" { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}{}>", if t == "checkListItem" { " class=\"check\"" } else { "" }));
            while i < list.len() && list[i]["type"] == t {
                let it = &list[i];
                let mark = if t == "checkListItem" { if it["props"]["checked"] == true { "☑ " } else { "☐ " } } else { "" };
                out.push_str(&format!("<li>{mark}{}{}</li>", inline(ctx, &it["content"]), blocks(ctx, it["children"].as_array().map(|v| v.as_slice()).unwrap_or(&[]))));
                i += 1;
            }
            out.push_str(&format!("</{tag}>"));
            continue;
        }
        let p = &b["props"];
        let text = inline(ctx, &b["content"]);
        out.push_str(&match t {
            "heading" => {
                let l = p["level"].as_u64().unwrap_or(1).clamp(1, 3) + 1;
                format!("<h{l}>{text}</h{l}>")
            }
            "quote" => format!("<blockquote>{text}</blockquote>"),
            "callout" => format!("<div class=\"callout\"><span>{}</span><div>{text}</div></div>", esc(p["emoji"].as_str().unwrap_or("💡"))),
            "codeBlock" => format!("<pre><code>{}</code></pre>", esc(&b["content"].as_array().into_iter().flatten().filter_map(|x| x["text"].as_str()).collect::<String>())),
            "mermaid" => format!("<pre class=\"mermaid\">{}</pre>", esc(p["source"].as_str().unwrap_or(""))),
            "divider" => "<hr>".into(),
            "table" => table(ctx, &b["content"]),
            "image" => match ctx.asset(p["url"].as_str().unwrap_or("")) {
                Some(src) => format!("<figure><img src=\"{src}\" alt=\"{}\"><figcaption>{}</figcaption></figure>", esc(p["caption"].as_str().unwrap_or("")), esc(p["caption"].as_str().unwrap_or(""))),
                None => String::new(),
            },
            "video" | "audio" | "file" => match ctx.asset(p["url"].as_str().unwrap_or("")) {
                Some(src) if t == "video" => format!("<video controls src=\"{src}\"></video>"),
                Some(src) if t == "audio" => format!("<audio controls src=\"{src}\"></audio>"),
                Some(src) => format!("<p>📎 <a href=\"{src}\">{}</a></p>", esc(p["name"].as_str().unwrap_or("file"))),
                None => String::new(),
            },
            "pageLink" => {
                let id = p["pageId"].as_str().unwrap_or("");
                format!("<p class=\"pagelink\">📄 {}</p>", ctx.page_link(id, &ctx.title(id)))
            }
            "canvasEmbed" => {
                let id = p["canvasId"].as_str().unwrap_or("").to_string();
                match ctx.svgs.get(&id).filter(|_| ctx.exported.contains(&id)) {
                    Some(svg) => format!("<figure class=\"canvas\">{svg}</figure>"),
                    None => String::new(),
                }
            }
            "databaseView" => database(ctx, p["databaseId"].as_str().unwrap_or("")),
            "taskRef" => String::new(),
            _ => if text.is_empty() { String::new() } else { format!("<p>{text}</p>") },
        });
        if let Some(children) = b["children"].as_array().filter(|c| !c.is_empty()) {
            out.push_str(&format!("<div class=\"indent\">{}</div>", blocks(ctx, children)));
        }
        i += 1;
    }
    out
}

const CSS: &str = r#":root{color-scheme:light dark;--bg:#fff;--fg:#37352f;--muted:#787774;--line:rgba(55,53,47,.14);--accent:#2383e2}
@media (prefers-color-scheme:dark){:root{--bg:#191919;--fg:rgba(255,255,255,.86);--muted:rgba(255,255,255,.5);--line:rgba(255,255,255,.12);--accent:#4a9eed}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.65 -apple-system,BlinkMacSystemFont,"Sukhumvit Set",Thonburi,"Helvetica Neue",sans-serif}
main{max-width:760px;margin:0 auto;padding:40px 20px 120px}nav.crumbs{font-size:13px;color:var(--muted);margin-bottom:24px}a{color:var(--accent)}
h1{font-size:2.2em;line-height:1.2;margin:.2em 0 .6em}h2,h3,h4{margin:1.4em 0 .4em}blockquote{border-left:3px solid var(--fg);margin:1em 0;padding-left:14px}
.callout{display:flex;gap:10px;background:rgba(127,127,127,.1);padding:12px 14px;border-radius:6px;margin:1em 0}pre{background:rgba(127,127,127,.12);padding:12px;border-radius:6px;overflow:auto}
code{font-family:"SF Mono",Menlo,monospace;font-size:.9em}table{border-collapse:collapse;width:100%;margin:1em 0;font-size:14px}td,th{border:1px solid var(--line);padding:6px 8px;text-align:left;vertical-align:top}
img,video{max-width:100%;border-radius:6px}figure{margin:1em 0}figcaption{color:var(--muted);font-size:13px}ul.check{list-style:none;padding-left:4px}.indent{padding-left:24px}
.canvas svg{max-width:100%;height:auto}hr{border:0;border-top:1px solid var(--line)}ul.index li{margin:4px 0}footer{color:var(--muted);font-size:12px;margin-top:60px}"#;

fn page_html(title: &str, body: &str, crumbs: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"th\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><link rel=\"stylesheet\" href=\"style.css\"></head>\n<body><main>{crumbs}<h1>{}</h1>{body}<footer>Exported from Personal Workspace</footer></main></body></html>\n",
        esc(title),
        esc(title)
    )
}

/// Writes `<dest>/<name>/` with index.html, one HTML file per selected page, style.css and used assets.
pub fn export(ws: &Workspace, page_ids: &[String], include_children: bool, site_title: &str, dest_parent: &Path, svgs: &HashMap<String, String>) -> Result<Value> {
    if page_ids.is_empty() {
        return invalid("choose at least one page to publish");
    }
    let mut exported: Vec<String> = vec![];
    for id in page_ids {
        let ids = if include_children { pages::subtree_ids(&ws.conn, id)? } else { vec![id.clone()] };
        for x in ids {
            let m = pages::get_meta(&ws.conn, &x)?;
            if m.deleted_at.is_none() && !exported.contains(&x) {
                exported.push(x);
            }
        }
    }
    let set: HashSet<String> = exported.iter().cloned().collect();
    let titles: HashMap<String, String> = exported.iter().filter_map(|id| pages::get_meta(&ws.conn, id).ok().map(|m| (id.clone(), if m.title.is_empty() { "Untitled".into() } else { m.title }))).collect();
    let name = filesystem::sanitize_file_name(if site_title.trim().is_empty() { "site" } else { site_title });
    let final_dir = dest_parent.join(&name);
    if final_dir.exists() {
        return invalid(format!("“{name}” already exists in that folder — choose another folder or name"));
    }
    let tmp = dest_parent.join(format!(".{name}.partial"));
    let _ = fs::remove_dir_all(&tmp);
    let result = (|| -> Result<usize> {
        fs::create_dir_all(tmp.join("assets"))?;
        let mut ctx = Ctx { ws, exported: &set, titles: &titles, svgs, assets: HashSet::new() };
        let mut index = String::from("<ul class=\"index\">");
        for id in &exported {
            let m = pages::get_meta(&ws.conn, id)?;
            let crumbs: Vec<String> = pages::ancestors(&ws.conn, id)?
                .iter()
                .filter(|a| set.contains(&a.id))
                .map(|a| ctx.page_link(&a.id, &titles[&a.id]))
                .collect();
            let crumbs = format!("<nav class=\"crumbs\"><a href=\"index.html\">{}</a>{}</nav>", esc(site_title), crumbs.iter().map(|c| format!(" / {c}")).collect::<String>());
            let mut body = String::new();
            match m.kind.as_str() {
                "database" => body.push_str(&database(&mut ctx, id)),
                "canvas" => body.push_str(&svgs.get(id).map(|s| format!("<figure class=\"canvas\">{s}</figure>")).unwrap_or_default()),
                "record" => {
                    if let (Ok(r), Some(db)) = (databases::get_record(&ws.conn, id), databases::database_of_record(&ws.conn, id)?) {
                        let rows: String = databases::properties(&ws.conn, &db)?
                            .iter()
                            .filter(|p| p.ptype != "title")
                            .map(|p| format!("<tr><th>{}</th><td>{}</td></tr>", esc(&p.name), esc(&databases::record_display(p, &r))))
                            .collect();
                        body.push_str(&format!("<table>{rows}</table>"));
                    }
                }
                _ => {}
            }
            body.push_str(&blocks(&mut ctx, &pages::load_blocks(&ws.conn, id)?));
            let children: Vec<String> = pages::query_metas(&ws.conn, "WHERE p.parent_id=?1 AND p.deleted_at IS NULL AND p.kind<>'record' ORDER BY p.position", [id])?
                .into_iter()
                .filter(|c| set.contains(&c.id))
                .map(|c| format!("<li>{}</li>", ctx.page_link(&c.id, &titles[&c.id])))
                .collect();
            if !children.is_empty() {
                body.push_str(&format!("<h3>Pages</h3><ul>{}</ul>", children.join("")));
            }
            filesystem::atomic_write(&tmp.join(format!("{id}.html")), page_html(&titles[id], &body, &crumbs).as_bytes())?;
            let depth = pages::ancestors(&ws.conn, id)?.iter().filter(|a| set.contains(&a.id)).count();
            index.push_str(&format!("<li style=\"margin-left:{}px\">{} {}</li>", depth * 20, esc(m.icon.as_deref().unwrap_or("")), ctx.page_link(id, &titles[id])));
        }
        index.push_str("</ul>");
        filesystem::atomic_write(&tmp.join("index.html"), page_html(site_title, &index, "").as_bytes())?;
        filesystem::atomic_write(&tmp.join("style.css"), CSS.as_bytes())?;
        for a in &ctx.assets {
            let src = filesystem::asset_path(&ws.root, a)?;
            if src.exists() {
                fs::copy(src, tmp.join("assets").join(a))?;
            }
        }
        Ok(ctx.assets.len())
    })();
    match result {
        Ok(assets) => {
            fs::rename(&tmp, &final_dir)?;
            Ok(json!({ "path": final_dir.to_string_lossy(), "index": final_dir.join("index.html").to_string_lossy(), "pages": exported.len(), "assets": assets }))
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            Err(e)
        }
    }
}

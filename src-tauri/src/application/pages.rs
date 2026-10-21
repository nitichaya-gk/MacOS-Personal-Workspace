//! Pages: tree structure, block content, links/backlinks, trash/archive and duplication.

use crate::domain::{extract_link_targets, extract_text, new_id, now, truncate_chars, PAGE_KINDS};
use crate::error::{invalid, not_found, AppError, Result};
use crate::infrastructure::search;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageMeta {
    pub id: String,
    pub section_id: Option<String>,
    pub parent_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub icon: Option<String>,
    pub position: f64,
    pub is_template: bool,
    pub favorite: bool,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_opened_at: Option<String>,
    pub canvas_kind: Option<String>,
}

pub const META_SELECT: &str = "SELECT p.id, p.section_id, p.parent_id, p.kind, p.title, p.icon, p.position, p.is_template, p.favorite,
    p.archived_at, p.deleted_at, p.created_at, p.updated_at, p.last_opened_at, c.kind
    FROM pages p LEFT JOIN canvases c ON c.id = p.id";

pub fn row_to_meta(r: &Row) -> rusqlite::Result<PageMeta> {
    Ok(PageMeta {
        id: r.get(0)?,
        section_id: r.get(1)?,
        parent_id: r.get(2)?,
        kind: r.get(3)?,
        title: r.get(4)?,
        icon: r.get(5)?,
        position: r.get(6)?,
        is_template: r.get::<_, i64>(7)? != 0,
        favorite: r.get::<_, i64>(8)? != 0,
        archived_at: r.get(9)?,
        deleted_at: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
        last_opened_at: r.get(13)?,
        canvas_kind: r.get(14)?,
    })
}

pub fn query_metas(conn: &Connection, where_sql: &str, args: impl rusqlite::Params) -> Result<Vec<PageMeta>> {
    let mut stmt = conn.prepare(&format!("{META_SELECT} {where_sql}"))?;
    let rows = stmt.query_map(args, row_to_meta)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn get_meta(conn: &Connection, id: &str) -> Result<PageMeta> {
    conn.query_row(&format!("{META_SELECT} WHERE p.id = ?1"), [id], row_to_meta)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("page {id}")))
}

pub fn exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.query_row("SELECT 1 FROM pages WHERE id=?1 AND deleted_at IS NULL", [id], |_| Ok(())).optional()?.is_some())
}

/// The page and all its descendants, parents before children.
pub fn subtree_ids(conn: &Connection, id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE sub(id, depth) AS (SELECT ?1, 0 UNION ALL SELECT p.id, sub.depth + 1 FROM pages p JOIN sub ON p.parent_id = sub.id)
         SELECT id FROM sub ORDER BY depth",
    )?;
    let rows = stmt.query_map([id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Ancestors from the root down to the direct parent.
pub fn ancestors(conn: &Connection, id: &str) -> Result<Vec<PageMeta>> {
    let mut out = vec![];
    let mut cur = get_meta(conn, id)?.parent_id;
    while let Some(pid) = cur {
        if out.len() > 10_000 {
            break; // corrupted data; never loop forever
        }
        let m = get_meta(conn, &pid)?;
        cur = m.parent_id.clone();
        out.push(m);
    }
    out.reverse();
    Ok(out)
}

pub fn project_of(conn: &Connection, id: &str) -> Result<Option<String>> {
    let me = get_meta(conn, id)?;
    if me.kind == "project" {
        return Ok(Some(me.id));
    }
    Ok(ancestors(conn, id)?.into_iter().rev().find(|a| a.kind == "project").map(|a| a.id))
}

fn default_section(conn: &Connection, kind: &str) -> Result<String> {
    let wanted = match kind {
        "project" => "projects",
        "database" => "databases",
        _ => "knowledge",
    };
    conn.query_row("SELECT id FROM sections WHERE kind=?1 ORDER BY position LIMIT 1", [wanted], |r| r.get(0))
        .optional()?
        .or(conn.query_row("SELECT id FROM sections ORDER BY position LIMIT 1", [], |r| r.get(0)).optional()?)
        .ok_or_else(|| AppError::Invalid("workspace has no sections".into()))
}

fn next_position(conn: &Connection, parent_id: Option<&str>, section_id: Option<&str>) -> Result<f64> {
    let max: Option<f64> = match parent_id {
        Some(p) => conn.query_row("SELECT MAX(position) FROM pages WHERE parent_id=?1", [p], |r| r.get(0))?,
        None => conn.query_row("SELECT MAX(position) FROM pages WHERE parent_id IS NULL AND section_id IS ?1", [section_id], |r| r.get(0))?,
    };
    Ok(max.map(|m| m + 1.0).unwrap_or(0.0))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CreatePage {
    pub parent_id: Option<String>,
    pub section_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub icon: Option<String>,
    pub canvas_kind: Option<String>,
}

pub fn create(conn: &Connection, p: CreatePage) -> Result<PageMeta> {
    let kind = if p.kind.is_empty() { "page".to_string() } else { p.kind.clone() };
    if !PAGE_KINDS.contains(&kind.as_str()) || kind == "record" {
        return invalid(format!("cannot create a page of kind {kind}"));
    }
    let section_id = match &p.parent_id {
        Some(pid) => {
            let parent = get_meta(conn, pid)?;
            if parent.deleted_at.is_some() {
                return invalid("parent page is in the trash");
            }
            if parent.kind == "database" {
                return invalid("add records to a database through the database");
            }
            parent.section_id
        }
        None => Some(match &p.section_id {
            Some(s) => s.clone(),
            None => default_section(conn, &kind)?,
        }),
    };
    let id = new_id();
    let ts = now();
    let tx = crate::infrastructure::database::begin(conn)?;
    let pos = next_position(&tx, p.parent_id.as_deref(), section_id.as_deref())?;
    let in_templates: bool = match &section_id {
        Some(s) => tx.query_row("SELECT kind='templates' FROM sections WHERE id=?1", [s], |r| r.get(0)).optional()?.unwrap_or(false),
        None => false,
    };
    tx.execute(
        "INSERT INTO pages(id, section_id, parent_id, kind, title, icon, position, is_template, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
        params![id, section_id, p.parent_id, kind, p.title.trim(), p.icon, pos, in_templates as i64, ts],
    )?;
    match kind.as_str() {
        "database" => super::databases::init(&tx, &id)?,
        "canvas" => {
            let ck = p.canvas_kind.as_deref().unwrap_or("diagram");
            if !matches!(ck, "diagram" | "wireframe") {
                return invalid("canvas kind must be diagram or wireframe");
            }
            tx.execute("INSERT INTO canvases(id, kind, updated_at) VALUES (?1,?2,?3)", params![id, ck, ts])?;
        }
        _ => {}
    }
    tx.commit()?;
    index(conn, &id)?;
    get_meta(conn, &id)
}

pub fn update(conn: &Connection, id: &str, patch: &Value) -> Result<PageMeta> {
    get_meta(conn, id)?;
    let ts = now();
    if let Some(t) = patch.get("title").and_then(|t| t.as_str()) {
        conn.execute("UPDATE pages SET title=?2, updated_at=?3 WHERE id=?1", params![id, t.trim(), ts])?;
    }
    if let Some(icon) = patch.get("icon") {
        conn.execute("UPDATE pages SET icon=?2, updated_at=?3 WHERE id=?1", params![id, icon.as_str(), ts])?;
    }
    if let Some(f) = patch.get("favorite").and_then(|f| f.as_bool()) {
        conn.execute("UPDATE pages SET favorite=?2 WHERE id=?1", params![id, f as i64])?;
    }
    index(conn, id)?;
    get_meta(conn, id)
}

/// Moves a page under a new parent (or to a section root). Refuses moves that would create a cycle.
pub fn move_page(conn: &Connection, id: &str, parent_id: Option<&str>, section_id: Option<&str>, index_at: Option<usize>) -> Result<PageMeta> {
    let me = get_meta(conn, id)?;
    if me.kind == "record" {
        return invalid("records stay inside their database");
    }
    let new_section = match parent_id {
        Some(pid) => {
            if pid == id {
                return invalid("a page cannot be inside itself");
            }
            let parent = get_meta(conn, pid)?;
            if parent.kind == "database" || parent.kind == "record" && me.kind == "database" {
                return invalid("cannot move a page into a database");
            }
            if ancestors(conn, pid)?.iter().any(|a| a.id == id) {
                return invalid("cannot move a page into one of its own subpages");
            }
            parent.section_id
        }
        None => section_id.map(String::from).or(me.section_id.clone()),
    };
    let tx = crate::infrastructure::database::begin(conn)?;
    let mut siblings: Vec<String> = {
        let mut stmt = match parent_id {
            Some(_) => tx.prepare("SELECT id FROM pages WHERE parent_id=?1 AND id<>?2 AND kind<>'record' ORDER BY position")?,
            None => tx.prepare("SELECT id FROM pages WHERE parent_id IS NULL AND section_id IS ?1 AND id<>?2 ORDER BY position")?,
        };
        let key = parent_id.map(String::from).or(new_section.clone());
        let rows = stmt.query_map(params![key, id], |r| r.get(0))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    let at = index_at.unwrap_or(siblings.len()).min(siblings.len());
    siblings.insert(at, id.to_string());
    for (i, sid) in siblings.iter().enumerate() {
        tx.execute("UPDATE pages SET position=?2 WHERE id=?1", params![sid, i as f64])?;
    }
    tx.execute("UPDATE pages SET parent_id=?2, section_id=?3, updated_at=?4 WHERE id=?1", params![id, parent_id, new_section, now()])?;
    for sid in subtree_ids(&tx, id)? {
        tx.execute("UPDATE pages SET section_id=?2 WHERE id=?1", params![sid, new_section])?;
        search::set_project(&tx, &sid, project_of(&tx, &sid)?.as_deref())?;
    }
    tx.commit()?;
    get_meta(conn, id)
}

pub fn list_tree(conn: &Connection) -> Result<Vec<PageMeta>> {
    query_metas(conn, "WHERE p.deleted_at IS NULL AND p.archived_at IS NULL AND p.kind <> 'record' ORDER BY p.position", [])
}

pub fn list_recent(conn: &Connection, limit: usize) -> Result<Vec<PageMeta>> {
    query_metas(
        conn,
        "WHERE p.deleted_at IS NULL AND p.last_opened_at IS NOT NULL AND p.is_template = 0 ORDER BY p.last_opened_at DESC LIMIT ?1",
        [limit as i64],
    )
}

/// Roots of trashed subtrees (children deleted together with their parent are not listed separately).
pub fn list_trash(conn: &Connection) -> Result<Vec<PageMeta>> {
    query_metas(
        conn,
        "LEFT JOIN pages pp ON pp.id = p.parent_id
         WHERE p.deleted_at IS NOT NULL AND (pp.id IS NULL OR pp.deleted_at IS NULL OR pp.deleted_at <> p.deleted_at)
         ORDER BY p.deleted_at DESC",
        [],
    )
}

pub fn list_archive(conn: &Connection) -> Result<Vec<PageMeta>> {
    query_metas(
        conn,
        "LEFT JOIN pages pp ON pp.id = p.parent_id
         WHERE p.deleted_at IS NULL AND p.archived_at IS NOT NULL AND (pp.id IS NULL OR pp.archived_at IS NULL OR pp.archived_at <> p.archived_at)
         ORDER BY p.archived_at DESC",
        [],
    )
}

fn stamp_subtree(conn: &Connection, id: &str, column: &str, value: Option<&str>, only_if: Option<&str>) -> Result<()> {
    let tx = crate::infrastructure::database::begin(conn)?;
    for sid in subtree_ids(&tx, id)? {
        match only_if {
            // Restoring only clears items stamped in the same operation, so earlier deletions stay in the trash.
            Some(prev) => tx.execute(&format!("UPDATE pages SET {column}=?2 WHERE id=?1 AND {column}=?3"), params![sid, value, prev])?,
            None => tx.execute(&format!("UPDATE pages SET {column}=?2 WHERE id=?1 AND {column} IS NULL"), params![sid, value])?,
        };
    }
    tx.commit()?;
    Ok(())
}

pub fn trash(conn: &Connection, id: &str) -> Result<()> {
    get_meta(conn, id)?;
    stamp_subtree(conn, id, "deleted_at", Some(&now()), None)
}

pub fn restore(conn: &Connection, id: &str) -> Result<PageMeta> {
    let me = get_meta(conn, id)?;
    let Some(stamp) = me.deleted_at.clone() else { return Ok(me) };
    stamp_subtree(conn, id, "deleted_at", None, Some(&stamp))?;
    // If the original parent is gone or still trashed, restore to the section root.
    if let Some(pid) = &me.parent_id {
        let parent_ok = exists(conn, pid)?;
        if !parent_ok && me.kind != "record" {
            let section = match &me.section_id {
                Some(s) if conn.query_row("SELECT 1 FROM sections WHERE id=?1", [s], |_| Ok(())).optional()?.is_some() => s.clone(),
                _ => default_section(conn, &me.kind)?,
            };
            conn.execute("UPDATE pages SET parent_id=NULL, section_id=?2 WHERE id=?1", params![id, section])?;
        } else if !parent_ok {
            return invalid("restore the database first");
        }
    }
    get_meta(conn, id)
}

pub fn delete_forever(conn: &Connection, id: &str) -> Result<()> {
    let me = get_meta(conn, id)?;
    if me.deleted_at.is_none() {
        return invalid("move the page to the trash first");
    }
    let tx = crate::infrastructure::database::begin(conn)?;
    for sid in subtree_ids(&tx, id)? {
        search::remove(&tx, &sid)?;
        tx.execute("DELETE FROM links WHERE source_id=?1", [&sid])?;
    }
    tx.execute("DELETE FROM pages WHERE id=?1", [id])?;
    tx.commit()?;
    Ok(())
}

pub fn empty_trash(conn: &Connection) -> Result<usize> {
    let roots = list_trash(conn)?;
    for p in &roots {
        delete_forever(conn, &p.id)?;
    }
    let tasks = conn.execute("DELETE FROM tasks WHERE deleted_at IS NOT NULL", [])?;
    Ok(roots.len() + tasks)
}

pub fn archive(conn: &Connection, id: &str) -> Result<()> {
    stamp_subtree(conn, id, "archived_at", Some(&now()), None)
}

pub fn unarchive(conn: &Connection, id: &str) -> Result<()> {
    let me = get_meta(conn, id)?;
    if let Some(stamp) = me.archived_at {
        stamp_subtree(conn, id, "archived_at", None, Some(&stamp))?;
    }
    Ok(())
}

// ---------- blocks ----------

struct BlockRow {
    id: String,
    parent: Option<String>,
    position: i64,
    btype: String,
    props: String,
    content: String,
    text: String,
}

fn flatten(blocks: &[Value], parent: Option<&str>, out: &mut Vec<BlockRow>) -> Result<()> {
    for (i, b) in blocks.iter().enumerate() {
        let id = b.get("id").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(new_id);
        let btype = b.get("type").and_then(|v| v.as_str()).unwrap_or("paragraph").to_string();
        let props = b.get("props").cloned().unwrap_or(json!({}));
        let content = b.get("content").cloned().unwrap_or(Value::Null);
        let mut text = String::new();
        extract_text(&content, &mut text);
        extract_text(&json!({ "p": props.clone() }), &mut text);
        out.push(BlockRow {
            id: id.clone(),
            parent: parent.map(String::from),
            position: i as i64,
            btype,
            props: serde_json::to_string(&props)?,
            content: serde_json::to_string(&content)?,
            text,
        });
        if let Some(children) = b.get("children").and_then(|c| c.as_array()) {
            flatten(children, Some(&id), out)?;
        }
    }
    Ok(())
}

/// Loads the block tree in BlockNote's document shape.
pub fn load_blocks(conn: &Connection, page_id: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare("SELECT id, parent_block_id, type, props, content FROM blocks WHERE page_id=?1 ORDER BY position")?;
    let rows: Vec<(String, Option<String>, String, String, String)> = stmt
        .query_map([page_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut by_parent: HashMap<Option<String>, Vec<(String, Value)>> = HashMap::new();
    for (id, parent, btype, props, content) in rows {
        let mut obj = Map::new();
        obj.insert("id".into(), json!(id));
        obj.insert("type".into(), json!(btype));
        obj.insert("props".into(), serde_json::from_str(&props).unwrap_or(json!({})));
        let c: Value = serde_json::from_str(&content).unwrap_or(Value::Null);
        if !c.is_null() {
            obj.insert("content".into(), c);
        }
        by_parent.entry(parent).or_default().push((id, Value::Object(obj)));
    }
    fn build(parent: Option<String>, by_parent: &mut HashMap<Option<String>, Vec<(String, Value)>>, depth: usize) -> Vec<Value> {
        let Some(items) = by_parent.remove(&parent) else { return vec![] };
        items
            .into_iter()
            .map(|(id, mut v)| {
                let children = if depth < 64 { build(Some(id), by_parent, depth + 1) } else { vec![] };
                v["children"] = Value::Array(children);
                v
            })
            .collect()
    }
    Ok(build(None, &mut by_parent, 0))
}

pub fn page_text(conn: &Connection, page_id: &str) -> Result<String> {
    let mut stmt = conn.prepare("SELECT text FROM blocks WHERE page_id=?1 ORDER BY parent_block_id IS NOT NULL, position")?;
    let parts: Vec<String> = stmt.query_map([page_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    Ok(parts.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n"))
}

/// Autosave entry point: keeps a periodic revision of the previous content, then saves.
pub fn save_blocks(conn: &Connection, page_id: &str, blocks: &Value) -> Result<String> {
    super::versions::maybe_snapshot(conn, page_id)?;
    save_blocks_raw(conn, page_id, blocks)
}

pub fn save_blocks_raw(conn: &Connection, page_id: &str, blocks: &Value) -> Result<String> {
    let me = get_meta(conn, page_id)?;
    if me.deleted_at.is_some() {
        return invalid("page is in the trash");
    }
    let Some(list) = blocks.as_array() else { return invalid("blocks must be a list") };
    let mut rows = vec![];
    flatten(list, None, &mut rows)?;
    let ts = now();
    let tx = crate::infrastructure::database::begin(conn)?;
    tx.execute("DELETE FROM blocks WHERE page_id=?1", [page_id])?;
    {
        let mut ins = tx.prepare(
            "INSERT OR REPLACE INTO blocks(page_id, id, parent_block_id, position, type, props, content, text) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        )?;
        for r in &rows {
            ins.execute(params![page_id, r.id, r.parent, r.position, r.btype, r.props, r.content, r.text])?;
        }
    }
    let mut targets: Vec<(String, String)> = vec![];
    for r in &rows {
        let mut ids = vec![];
        let v = json!({ "p": serde_json::from_str::<Value>(&r.props)?, "c": serde_json::from_str::<Value>(&r.content)? });
        extract_link_targets(&v, &mut ids);
        for id in ids {
            if id != page_id && !targets.iter().any(|(t, _)| t == &id) {
                targets.push((id, truncate_chars(&r.text, 160)));
            }
        }
    }
    set_links(&tx, page_id, "page", &targets)?;
    tx.execute("UPDATE pages SET updated_at=?2 WHERE id=?1", params![page_id, ts])?;
    tx.commit()?;
    index(conn, page_id)?;
    Ok(ts)
}

pub fn set_links(conn: &Connection, source_id: &str, source_type: &str, targets: &[(String, String)]) -> Result<()> {
    conn.execute("DELETE FROM links WHERE source_id=?1", [source_id])?;
    let mut ins = conn.prepare("INSERT OR IGNORE INTO links(source_id, source_type, target_id, context) VALUES (?1,?2,?3,?4)")?;
    for (t, ctx) in targets {
        ins.execute(params![source_id, source_type, t, ctx])?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backlink {
    pub source_id: String,
    pub source_type: String,
    pub title: String,
    pub icon: Option<String>,
    pub kind: Option<String>,
    pub context: String,
}

pub fn backlinks(conn: &Connection, id: &str) -> Result<Vec<Backlink>> {
    let mut stmt = conn.prepare(
        "SELECT l.source_id, l.source_type, COALESCE(p.title, t.title, ''), p.icon, p.kind, l.context
         FROM links l LEFT JOIN pages p ON p.id = l.source_id LEFT JOIN tasks t ON t.id = l.source_id
         WHERE l.target_id = ?1 AND ((p.id IS NOT NULL AND p.deleted_at IS NULL) OR (t.id IS NOT NULL AND t.deleted_at IS NULL))
         ORDER BY COALESCE(p.updated_at, t.updated_at) DESC",
    )?;
    let rows = stmt.query_map([id], |r| {
        Ok(Backlink { source_id: r.get(0)?, source_type: r.get(1)?, title: r.get(2)?, icon: r.get(3)?, kind: r.get(4)?, context: r.get(5)? })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDetail {
    pub meta: PageMeta,
    pub blocks: Vec<Value>,
    pub breadcrumb: Vec<PageMeta>,
    pub project_id: Option<String>,
    pub section_name: Option<String>,
}

pub fn open(conn: &Connection, id: &str) -> Result<PageDetail> {
    let meta = get_meta(conn, id)?;
    conn.execute("UPDATE pages SET last_opened_at=?2 WHERE id=?1", params![id, now()])?;
    let section_name = match &meta.section_id {
        Some(s) => conn.query_row("SELECT name FROM sections WHERE id=?1", [s], |r| r.get(0)).optional()?,
        None => None,
    };
    Ok(PageDetail {
        blocks: load_blocks(conn, id)?,
        breadcrumb: ancestors(conn, id)?,
        project_id: project_of(conn, id)?,
        section_name,
        meta,
    })
}

/// Recomputes the search document of one page (title + text of its content).
pub fn index(conn: &Connection, id: &str) -> Result<()> {
    let me = get_meta(conn, id)?;
    let body = match me.kind.as_str() {
        "canvas" => super::canvases::canvas_text(conn, id)?,
        "database" => {
            let mut stmt = conn.prepare("SELECT name FROM database_properties WHERE database_id=?1")?;
            let names: Vec<String> = stmt.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            names.join(" ")
        }
        "record" => [super::databases::record_values_text(conn, id)?, page_text(conn, id)?]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => page_text(conn, id)?,
    };
    let project = project_of(conn, id)?;
    search::upsert(conn, id, &me.kind, project.as_deref(), &me.updated_at, &me.title, &body)
}

pub fn rebuild_index(conn: &Connection) -> Result<usize> {
    conn.execute("DELETE FROM search_docs", [])?;
    conn.execute("INSERT INTO search_fts(search_fts) VALUES('rebuild')", [])?;
    let ids: Vec<String> = conn.prepare("SELECT id FROM pages")?.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    for id in &ids {
        index(conn, id)?;
    }
    let tasks: Vec<String> = conn.prepare("SELECT id FROM tasks")?.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    for id in &tasks {
        super::tasks::index(conn, id)?;
    }
    let goals: Vec<String> = conn.prepare("SELECT id FROM goals WHERE deleted_at IS NULL")?.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    for id in &goals {
        super::goals::index(conn, id)?;
    }
    Ok(ids.len() + tasks.len() + goals.len())
}

// ---------- duplication (used by Duplicate and Templates) ----------

fn remap(s: &str, map: &[(String, String)]) -> String {
    let mut out = s.to_string();
    for (old, new) in map {
        if out.contains(old.as_str()) {
            out = out.replace(old.as_str(), new);
        }
    }
    out
}

/// Deep-copies a page subtree (blocks, databases, records, canvases) with fresh ids.
/// Internal references inside the subtree are rewritten to the copies; the source is never modified.
pub fn duplicate_subtree(
    conn: &Connection,
    root_id: &str,
    target_parent: Option<&str>,
    target_section: Option<&str>,
    title: Option<&str>,
    is_template: Option<bool>,
) -> Result<String> {
    let root = get_meta(conn, root_id)?;
    let ids: Vec<String> = subtree_ids(conn, root_id)?
        .into_iter()
        .filter(|id| get_meta(conn, id).map(|m| m.deleted_at.is_none()).unwrap_or(false))
        .collect();
    let mut map: Vec<(String, String)> = ids.iter().map(|id| (id.clone(), new_id())).collect();
    let page_map: HashMap<String, String> = map.iter().cloned().collect();
    for id in &ids {
        let mut stmt = conn.prepare("SELECT id FROM database_properties WHERE database_id=?1 UNION ALL SELECT id FROM database_views WHERE database_id=?1")?;
        let extra: Vec<String> = stmt.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        map.extend(extra.into_iter().map(|e| (e, new_id())));
    }
    let id_of = |old: &str| map.iter().find(|(o, _)| o == old).map(|(_, n)| n.clone()).unwrap_or_else(|| old.to_string());

    let section = match target_parent {
        Some(p) => get_meta(conn, p)?.section_id,
        None => target_section.map(String::from).or(root.section_id.clone()),
    };
    let ts = now();
    let tx = crate::infrastructure::database::begin(conn)?;
    for old in &ids {
        let m = get_meta(&tx, old)?;
        let new = &page_map[old];
        let is_root = old == root_id;
        let parent = if is_root { target_parent.map(String::from) } else { m.parent_id.as_ref().map(|p| id_of(p)) };
        let position = if is_root { next_position(&tx, parent.as_deref(), section.as_deref())? } else { m.position };
        let t = if is_root { title.map(String::from).unwrap_or(m.title.clone()) } else { m.title.clone() };
        let tpl = if is_root { is_template.unwrap_or(m.is_template) } else { m.is_template };
        tx.execute(
            "INSERT INTO pages(id, section_id, parent_id, kind, title, icon, position, is_template, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
            params![new, section, parent, m.kind, t, m.icon, position, tpl as i64, ts],
        )?;
        tx.execute(
            "INSERT INTO blocks(page_id, id, parent_block_id, position, type, props, content, text)
             SELECT ?2, id, parent_block_id, position, type, props, content, text FROM blocks WHERE page_id=?1",
            params![old, new],
        )?;
        // Rewrite references inside the copied blocks.
        let blocks: Vec<(String, String, String)> = tx
            .prepare("SELECT id, props, content FROM blocks WHERE page_id=?1")?
            .query_map([new], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;
        for (bid, props, content) in blocks {
            tx.execute("UPDATE blocks SET props=?3, content=?4 WHERE page_id=?1 AND id=?2", params![new, bid, remap(&props, &map), remap(&content, &map)])?;
        }
        tx.execute(
            "INSERT INTO links(source_id, source_type, target_id, context) SELECT ?2, source_type, target_id, context FROM links WHERE source_id=?1",
            params![old, new],
        )?;
        for (o, n) in &map {
            tx.execute("UPDATE links SET target_id=?3 WHERE source_id=?1 AND target_id=?2", params![new, o, n])?;
        }
        match m.kind.as_str() {
            "database" => {
                tx.execute("INSERT INTO databases(id, mcp_access) SELECT ?2, mcp_access FROM databases WHERE id=?1", params![old, new])?;
                let props: Vec<(String, String, String, String, f64)> = tx
                    .prepare("SELECT id, name, type, config, position FROM database_properties WHERE database_id=?1")?
                    .query_map([old], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
                    .collect::<std::result::Result<_, _>>()?;
                for (pid, name, ptype, config, pos) in props {
                    tx.execute(
                        "INSERT INTO database_properties(id, database_id, name, type, config, position) VALUES (?1,?2,?3,?4,?5,?6)",
                        params![id_of(&pid), new, name, ptype, config, pos],
                    )?;
                }
                let views: Vec<(String, String, String, String, f64)> = tx
                    .prepare("SELECT id, name, type, config, position FROM database_views WHERE database_id=?1")?
                    .query_map([old], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
                    .collect::<std::result::Result<_, _>>()?;
                for (vid, name, vtype, config, pos) in views {
                    tx.execute(
                        "INSERT INTO database_views(id, database_id, name, type, config, position) VALUES (?1,?2,?3,?4,?5,?6)",
                        params![id_of(&vid), new, name, vtype, remap(&config, &map), pos],
                    )?;
                }
            }
            "canvas" => {
                tx.execute("INSERT INTO canvases(id, kind, app_state, updated_at) SELECT ?2, kind, app_state, ?3 FROM canvases WHERE id=?1", params![old, new, ts])?;
                let objs: Vec<(String, i64, String, String)> = tx
                    .prepare("SELECT id, position, type, data FROM canvas_objects WHERE canvas_id=?1")?
                    .query_map([old], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                    .collect::<std::result::Result<_, _>>()?;
                for (oid, pos, otype, data) in objs {
                    tx.execute(
                        "INSERT INTO canvas_objects(canvas_id, id, position, type, data) VALUES (?1,?2,?3,?4,?5)",
                        params![new, oid, pos, otype, remap(&data, &map)],
                    )?;
                }
            }
            _ => {}
        }
    }
    // Records last, once every database row they reference exists.
    for old in &ids {
        let rec: Option<(String, f64)> = tx
            .query_row("SELECT database_id, position FROM database_records WHERE id=?1", [old], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        if let Some((db, pos)) = rec {
            let new = &page_map[old];
            let new_db = id_of(&db);
            let pos = if old == root_id {
                tx.query_row("SELECT COALESCE(MAX(position)+1, 0) FROM database_records WHERE database_id=?1", [&new_db], |r| r.get(0))?
            } else {
                pos
            };
            tx.execute("INSERT INTO database_records(id, database_id, position) VALUES (?1,?2,?3)", params![new, new_db, pos])?;
            let vals: Vec<(String, String)> = tx
                .prepare("SELECT property_id, value FROM record_values WHERE record_id=?1")?
                .query_map([old], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?;
            for (pid, val) in vals {
                tx.execute("INSERT INTO record_values(record_id, property_id, value) VALUES (?1,?2,?3)", params![new, id_of(&pid), remap(&val, &map)])?;
            }
        }
    }
    tx.commit()?;
    for old in &ids {
        index(conn, &page_map[old])?;
    }
    Ok(page_map[root_id].clone())
}

pub fn duplicate(conn: &Connection, id: &str) -> Result<PageMeta> {
    let me = get_meta(conn, id)?;
    if me.kind == "record" {
        let new = duplicate_subtree(conn, id, me.parent_id.as_deref(), None, Some(&format!("{} (copy)", me.title)), None)?;
        return get_meta(conn, &new);
    }
    let new = duplicate_subtree(conn, id, me.parent_id.as_deref(), me.section_id.as_deref(), Some(&format!("{} (copy)", me.title)), None)?;
    get_meta(conn, &new)
}

pub fn not_in_trash(conn: &Connection, id: &str) -> Result<PageMeta> {
    let m = get_meta(conn, id)?;
    if m.deleted_at.is_some() {
        return not_found(format!("page {id}"));
    }
    Ok(m)
}

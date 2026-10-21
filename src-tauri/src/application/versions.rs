//! Page revision history: periodic snapshots on save, snapshots before MCP edits and restores.

use super::{export, pages};
use crate::domain::{new_id, now};
use crate::error::{AppError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

const AUTOSAVE_INTERVAL_MIN: i64 = 10;
const KEEP_PER_PAGE: i64 = 50;

/// Stores the page's current title + blocks.
pub fn snapshot(conn: &Connection, page_id: &str, source: &str) -> Result<()> {
    let meta = pages::get_meta(conn, page_id)?;
    let blocks = pages::load_blocks(conn, page_id)?;
    conn.execute(
        "INSERT INTO versions(id, page_id, snapshot, created_at, source, title) VALUES (?1,?2,?3,?4,?5,?6)",
        params![new_id(), page_id, json!({ "title": meta.title, "blocks": blocks }).to_string(), now(), source, meta.title],
    )?;
    conn.execute(
        "DELETE FROM versions WHERE page_id=?1 AND id NOT IN (SELECT id FROM versions WHERE page_id=?1 ORDER BY created_at DESC LIMIT ?2)",
        params![page_id, KEEP_PER_PAGE],
    )?;
    Ok(())
}

/// Before an autosave overwrites a page: keep the previous state if the last snapshot is old enough.
pub fn maybe_snapshot(conn: &Connection, page_id: &str) -> Result<()> {
    let has_blocks: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM blocks WHERE page_id=?1)", [page_id], |r| r.get(0))?;
    if !has_blocks {
        return Ok(());
    }
    let last: Option<String> = conn.query_row("SELECT MAX(created_at) FROM versions WHERE page_id=?1", [page_id], |r| r.get(0))?;
    let stale = match last.and_then(|l| chrono::DateTime::parse_from_rfc3339(&l).ok()) {
        Some(t) => chrono::Utc::now().signed_duration_since(t) > chrono::Duration::minutes(AUTOSAVE_INTERVAL_MIN),
        None => true,
    };
    if stale {
        snapshot(conn, page_id, "autosave")?;
    }
    Ok(())
}

pub fn list(conn: &Connection, page_id: &str) -> Result<Vec<Value>> {
    let rows = conn
        .prepare("SELECT id, created_at, source, title, length(snapshot) FROM versions WHERE page_id=?1 ORDER BY created_at DESC")?
        .query_map([page_id], |r| {
            Ok(json!({ "id": r.get::<_, String>(0)?, "createdAt": r.get::<_, String>(1)?, "source": r.get::<_, String>(2)?, "title": r.get::<_, String>(3)?, "size": r.get::<_, i64>(4)? }))
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

fn load(conn: &Connection, id: &str) -> Result<(String, Value)> {
    let (page, snap): (String, String) = conn
        .query_row("SELECT page_id, snapshot FROM versions WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?
        .ok_or_else(|| AppError::NotFound("version".into()))?;
    Ok((page, serde_json::from_str(&snap)?))
}

pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    let (page, snap) = load(conn, id)?;
    let blocks = snap["blocks"].as_array().cloned().unwrap_or_default();
    Ok(json!({ "id": id, "pageId": page, "title": snap["title"], "markdown": export::blocks_to_markdown(conn, snap["title"].as_str().unwrap_or(""), &blocks) }))
}

/// Restores a version; the current state is snapshotted first so the restore itself can be undone.
pub fn restore(conn: &Connection, id: &str) -> Result<Value> {
    let (page, snap) = load(conn, id)?;
    snapshot(conn, &page, "before_restore")?;
    if let Some(t) = snap["title"].as_str() {
        pages::update(conn, &page, &json!({ "title": t }))?;
    }
    pages::save_blocks_raw(conn, &page, &snap["blocks"])?;
    Ok(json!({ "pageId": page }))
}

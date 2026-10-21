//! Private comments on a page, optionally anchored to a block.

use crate::domain::{new_id, now};
use crate::error::{invalid, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub fn list(conn: &Connection, page_id: &str) -> Result<Vec<Value>> {
    let rows = conn
        .prepare("SELECT id, block_id, quote, body, resolved_at, created_at, updated_at FROM comments WHERE page_id=?1 ORDER BY resolved_at IS NOT NULL, created_at")?
        .query_map([page_id], |r| {
            Ok(json!({ "id": r.get::<_, String>(0)?, "blockId": r.get::<_, Option<String>>(1)?, "quote": r.get::<_, String>(2)?, "body": r.get::<_, String>(3)?,
                       "resolvedAt": r.get::<_, Option<String>>(4)?, "createdAt": r.get::<_, String>(5)?, "updatedAt": r.get::<_, String>(6)? }))
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

pub fn add(conn: &Connection, page_id: &str, block_id: Option<&str>, quote: &str, body: &str) -> Result<Value> {
    if body.trim().is_empty() {
        return invalid("write something first");
    }
    super::pages::get_meta(conn, page_id)?;
    let id = new_id();
    let ts = now();
    conn.execute(
        "INSERT INTO comments(id, page_id, block_id, quote, body, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?6)",
        params![id, page_id, block_id, crate::domain::truncate_chars(quote, 200), body.trim(), ts],
    )?;
    Ok(json!({ "id": id }))
}

pub fn update(conn: &Connection, id: &str, body: Option<&str>, resolved: Option<bool>) -> Result<()> {
    if let Some(b) = body {
        if b.trim().is_empty() {
            return invalid("comment cannot be empty");
        }
        conn.execute("UPDATE comments SET body=?2, updated_at=?3 WHERE id=?1", params![id, b.trim(), now()])?;
    }
    if let Some(r) = resolved {
        conn.execute("UPDATE comments SET resolved_at=?2 WHERE id=?1", params![id, r.then(now)])?;
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM comments WHERE id=?1", [id])?;
    Ok(())
}

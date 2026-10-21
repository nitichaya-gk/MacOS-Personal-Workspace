//! Sidebar sections (Projects, Knowledge Base, Databases, Templates and user-created ones).

use crate::domain::{new_id, now};
use crate::error::{invalid, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub kind: String,
    pub position: f64,
}

pub fn list(conn: &Connection) -> Result<Vec<Section>> {
    let mut stmt = conn.prepare("SELECT id, name, icon, kind, position FROM sections ORDER BY position")?;
    let rows = stmt.query_map([], |r| Ok(Section { id: r.get(0)?, name: r.get(1)?, icon: r.get(2)?, kind: r.get(3)?, position: r.get(4)? }))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn create(conn: &Connection, name: &str, icon: Option<&str>) -> Result<Section> {
    if name.trim().is_empty() {
        return invalid("section name is required");
    }
    let pos: f64 = conn.query_row("SELECT COALESCE(MAX(position)+1, 0) FROM sections", [], |r| r.get(0))?;
    let id = new_id();
    conn.execute(
        "INSERT INTO sections(id, name, icon, kind, position, created_at) VALUES (?1,?2,?3,'custom',?4,?5)",
        params![id, name.trim(), icon, pos, now()],
    )?;
    Ok(list(conn)?.into_iter().find(|s| s.id == id).unwrap())
}

pub fn update(conn: &Connection, id: &str, name: Option<&str>, icon: Option<&str>, position: Option<f64>) -> Result<()> {
    if let Some(n) = name {
        if n.trim().is_empty() {
            return invalid("section name is required");
        }
        conn.execute("UPDATE sections SET name=?2 WHERE id=?1", params![id, n.trim()])?;
    }
    if let Some(i) = icon {
        conn.execute("UPDATE sections SET icon=?2 WHERE id=?1", params![id, i])?;
    }
    if let Some(p) = position {
        conn.execute("UPDATE sections SET position=?2 WHERE id=?1", params![id, p])?;
    }
    Ok(())
}

/// Only empty custom sections can be removed, so nothing is ever orphaned.
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let (kind, used): (String, i64) = conn.query_row(
        "SELECT kind, (SELECT COUNT(*) FROM pages WHERE section_id=?1) FROM sections WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if kind != "custom" {
        return invalid("built-in sections can be renamed but not deleted");
    }
    if used > 0 {
        return invalid("move or delete the pages in this section first");
    }
    conn.execute("DELETE FROM sections WHERE id=?1", [id])?;
    Ok(())
}

//! Page templates live in the Templates section. Using one deep-copies it; the original never changes.

use super::pages::{self, PageMeta};
use crate::error::{invalid, Result};
use rusqlite::Connection;

pub fn templates_section(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT id FROM sections WHERE kind='templates' ORDER BY position LIMIT 1", [], |r| r.get(0))?)
}

pub fn list(conn: &Connection) -> Result<Vec<PageMeta>> {
    pages::query_metas(
        conn,
        "JOIN sections s ON s.id = p.section_id WHERE s.kind='templates' AND p.parent_id IS NULL AND p.deleted_at IS NULL AND p.archived_at IS NULL ORDER BY p.position",
        [],
    )
}

pub fn instantiate(conn: &Connection, template_id: &str, parent_id: Option<&str>, section_id: Option<&str>, title: Option<&str>) -> Result<PageMeta> {
    let t = pages::get_meta(conn, template_id)?;
    if t.kind == "record" {
        return invalid("use the database's New button for record templates");
    }
    let section = match (parent_id, section_id) {
        (Some(_), _) => None,
        (None, Some(s)) => Some(s.to_string()),
        (None, None) => Some(conn.query_row(
            "SELECT id FROM sections WHERE kind = CASE ?1 WHEN 'project' THEN 'projects' WHEN 'database' THEN 'databases' ELSE 'knowledge' END ORDER BY position LIMIT 1",
            [&t.kind],
            |r| r.get(0),
        )?),
    };
    let id = pages::duplicate_subtree(conn, template_id, parent_id, section.as_deref(), title, Some(false))?;
    // Pages copied out of the Templates section are ordinary pages.
    conn.execute(
        "UPDATE pages SET is_template=0 WHERE id IN (WITH RECURSIVE sub(id) AS (SELECT ?1 UNION ALL SELECT p.id FROM pages p JOIN sub ON p.parent_id=sub.id) SELECT id FROM sub)
         AND kind <> 'record'",
        [&id],
    )?;
    pages::get_meta(conn, &id)
}

pub fn save_as_template(conn: &Connection, page_id: &str) -> Result<PageMeta> {
    let section = templates_section(conn)?;
    let id = pages::duplicate_subtree(conn, page_id, None, Some(&section), None, Some(true))?;
    conn.execute(
        "UPDATE pages SET is_template=1 WHERE id IN (WITH RECURSIVE sub(id) AS (SELECT ?1 UNION ALL SELECT p.id FROM pages p JOIN sub ON p.parent_id=sub.id) SELECT id FROM sub)
         AND kind <> 'record'",
        [&id],
    )?;
    pages::get_meta(conn, &id)
}

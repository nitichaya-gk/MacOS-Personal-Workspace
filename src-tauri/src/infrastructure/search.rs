//! Full-text search over pages, records, canvases and tasks.
//! The index is derived data: `rebuild` regenerates it from source tables.

use crate::error::Result;
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};

pub fn upsert(conn: &Connection, entity_id: &str, entity_type: &str, project_id: Option<&str>, updated_at: &str, title: &str, body: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO search_docs(entity_id, entity_type, project_id, updated_at, title, body) VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(entity_id) DO UPDATE SET entity_type=excluded.entity_type, project_id=excluded.project_id,
           updated_at=excluded.updated_at, title=excluded.title, body=excluded.body",
        params![entity_id, entity_type, project_id, updated_at, title, body],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, entity_id: &str) -> Result<()> {
    conn.execute("DELETE FROM search_docs WHERE entity_id = ?1", [entity_id])?;
    Ok(())
}

pub fn set_project(conn: &Connection, entity_id: &str, project_id: Option<&str>) -> Result<()> {
    conn.execute("UPDATE search_docs SET project_id = ?2 WHERE entity_id = ?1", params![entity_id, project_id])?;
    Ok(())
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchFilters {
    /// page | project | database | record | canvas | task
    pub types: Vec<String>,
    pub project_id: Option<String>,
    /// ISO date/time; only items modified at or after it.
    pub modified_after: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub entity_id: String,
    pub entity_type: String,
    pub title: String,
    pub snippet: String,
    pub project_id: Option<String>,
    pub updated_at: String,
    pub icon: Option<String>,
}

pub fn search(conn: &Connection, query: &str, f: &SearchFilters) -> Result<Vec<SearchHit>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let limit = f.limit.unwrap_or(50).clamp(1, 200);
    // Trigram matching needs ≥3 characters per term; shorter terms fall back to LIKE.
    let terms: Vec<&str> = q.split_whitespace().collect();
    let (long, short): (Vec<&str>, Vec<&str>) = terms.iter().partition(|t| t.chars().count() >= 3);

    let mut sql = String::from(
        "SELECT d.entity_id, d.entity_type, d.title, d.body, d.project_id, d.updated_at, p.icon
         FROM search_docs d
         LEFT JOIN pages p ON p.id = d.entity_id
         LEFT JOIN tasks t ON t.id = d.entity_id
         LEFT JOIN goals g ON g.id = d.entity_id
         WHERE ((p.id IS NOT NULL AND p.deleted_at IS NULL AND p.is_template = 0
           AND (p.section_id IS NULL OR p.section_id NOT IN (SELECT id FROM sections WHERE kind='templates'))) OR (t.id IS NOT NULL AND t.deleted_at IS NULL) OR (g.id IS NOT NULL AND g.deleted_at IS NULL))",
    );
    let mut args: Vec<SqlValue> = vec![];
    if !long.is_empty() {
        let fts = long.iter().map(|t| format!("\"{}\"", t.replace('"', "\"\""))).collect::<Vec<_>>().join(" AND ");
        sql.push_str(" AND d.rowid IN (SELECT rowid FROM search_fts WHERE search_fts MATCH ?)");
        args.push(SqlValue::Text(fts));
    }
    for t in &short {
        sql.push_str(" AND (d.title LIKE ? ESCAPE '\\' OR d.body LIKE ? ESCAPE '\\')");
        let pat = format!("%{}%", t.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        args.push(SqlValue::Text(pat.clone()));
        args.push(SqlValue::Text(pat));
    }
    if !f.types.is_empty() {
        sql.push_str(&format!(" AND d.entity_type IN ({})", vec!["?"; f.types.len()].join(",")));
        args.extend(f.types.iter().map(|t| SqlValue::Text(t.clone())));
    }
    if let Some(p) = &f.project_id {
        sql.push_str(" AND (d.project_id = ? OR d.entity_id = ?)");
        args.push(SqlValue::Text(p.clone()));
        args.push(SqlValue::Text(p.clone()));
    }
    if let Some(m) = &f.modified_after {
        sql.push_str(" AND d.updated_at >= ?");
        args.push(SqlValue::Text(m.clone()));
    }
    // Title matches first, then most recently edited.
    sql.push_str(" ORDER BY (d.title LIKE ?) DESC, d.updated_at DESC LIMIT ?");
    args.push(SqlValue::Text(format!("%{}%", terms[0])));
    args.push(SqlValue::Integer(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args), |r| {
        let body: String = r.get(3)?;
        Ok(SearchHit {
            entity_id: r.get(0)?,
            entity_type: r.get(1)?,
            title: r.get(2)?,
            snippet: crate::domain::snippet(&body, terms[0], 60),
            project_id: r.get(4)?,
            updated_at: r.get(5)?,
            icon: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM search_docs", [], |r| r.get(0))?)
}

//! Local forms built from a database's properties. A valid submission creates exactly one record.

use super::databases::{self, CreateRecord};
use crate::domain::properties::{is_computed, validate_value};
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};

fn row(conn: &Connection, id: &str) -> Result<(String, String, Value)> {
    conn.query_row("SELECT database_id, name, config FROM forms WHERE id=?1", [id], |r| {
        Ok((r.get(0)?, r.get(1)?, serde_json::from_str::<Value>(&r.get::<_, String>(2)?).unwrap_or(json!({}))))
    })
    .optional()?
    .ok_or_else(|| AppError::NotFound("form".into()))
}

pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    let (db_id, name, config) = row(conn, id)?;
    let db = databases::get(conn, &db_id)?;
    Ok(json!({ "id": id, "databaseId": db_id, "databaseTitle": db.title, "name": name, "config": config, "properties": db.properties }))
}

pub fn list(conn: &Connection, db_id: &str) -> Result<Vec<Value>> {
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT id, name FROM forms WHERE database_id=?1 ORDER BY created_at")?
        .query_map([db_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows.into_iter().map(|(id, name)| json!({ "id": id, "name": name })).collect())
}

/// New form with the title (required) plus every editable property (optional).
pub fn create(conn: &Connection, db_id: &str, name: &str) -> Result<Value> {
    let db = databases::get(conn, db_id)?;
    let fields: Vec<Value> = db
        .properties
        .iter()
        .filter(|p| p.ptype == "title" || !is_computed(&p.ptype))
        .map(|p| json!({ "propertyId": p.id, "label": p.name, "required": p.ptype == "title", "help": "" }))
        .collect();
    let config = json!({ "description": "", "fields": fields, "submitLabel": "Submit", "successMessage": "Thanks — your response was saved." });
    let id = new_id();
    let ts = now();
    conn.execute(
        "INSERT INTO forms(id, database_id, name, config, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
        params![id, db_id, if name.trim().is_empty() { "Form" } else { name.trim() }, config.to_string(), ts],
    )?;
    get(conn, &id)
}

pub fn update(conn: &Connection, id: &str, patch: &Value) -> Result<Value> {
    let (db_id, _, _) = row(conn, id)?;
    if let Some(n) = patch.get("name").and_then(|v| v.as_str()) {
        if n.trim().is_empty() {
            return invalid("form name is required");
        }
        conn.execute("UPDATE forms SET name=?2 WHERE id=?1", params![id, n.trim()])?;
    }
    if let Some(cfg) = patch.get("config") {
        let ps = databases::properties(conn, &db_id)?;
        let fields = cfg.get("fields").and_then(|f| f.as_array()).ok_or_else(|| AppError::Invalid("a form needs a list of fields".into()))?;
        for f in fields {
            let pid = f.get("propertyId").and_then(|v| v.as_str()).unwrap_or("");
            match ps.iter().find(|p| p.id == pid) {
                Some(p) if p.ptype == "title" || !is_computed(&p.ptype) => {}
                Some(_) => return invalid("computed properties cannot be form fields"),
                None => return invalid("a field refers to a property that no longer exists"),
            }
        }
        conn.execute("UPDATE forms SET config=?2 WHERE id=?1", params![id, cfg.to_string()])?;
    }
    conn.execute("UPDATE forms SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM forms WHERE id=?1", [id])?;
    Ok(())
}

fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Bool(b) => !b,
        _ => false,
    }
}

/// Validates every field first; only a fully valid submission creates the record.
pub fn submit(conn: &Connection, id: &str, values: &Map<String, Value>) -> Result<Value> {
    let (db_id, _, cfg) = row(conn, id)?;
    let ps = databases::properties(conn, &db_id)?;
    let mut errors = Map::new();
    let mut title = String::new();
    let mut clean = Map::new();
    for f in cfg.get("fields").and_then(|f| f.as_array()).into_iter().flatten() {
        let pid = f.get("propertyId").and_then(|v| v.as_str()).unwrap_or("");
        let Some(p) = ps.iter().find(|p| p.id == pid) else { continue };
        let label = f.get("label").and_then(|l| l.as_str()).filter(|l| !l.is_empty()).unwrap_or(&p.name).to_string();
        let v = values.get(pid).cloned().unwrap_or(Value::Null);
        let required = f.get("required").and_then(|r| r.as_bool()) == Some(true);
        if is_blank(&v) {
            if required {
                errors.insert(pid.into(), json!(format!("{label} is required")));
            }
            continue;
        }
        if p.ptype == "title" {
            match v.as_str() {
                Some(s) => title = s.trim().to_string(),
                None => {
                    errors.insert(pid.into(), json!(format!("{label} must be text")));
                }
            }
            continue;
        }
        match validate_value(&p.ptype, &p.config, &v) {
            Ok(Some(ok)) => {
                clean.insert(pid.into(), ok);
            }
            Ok(None) => {}
            Err(e) => {
                errors.insert(pid.into(), json!(format!("{label}: {e}")));
            }
        }
    }
    if !errors.is_empty() {
        return Ok(json!({ "ok": false, "errors": errors }));
    }
    let tx = crate::infrastructure::database::begin(conn)?;
    let rec = databases::create_record(&tx, &db_id, CreateRecord { title, values: clean, ..Default::default() })?;
    tx.commit()?;
    Ok(json!({ "ok": true, "record": rec, "message": cfg.get("successMessage") }))
}

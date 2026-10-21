//! Databases: properties, views, records and record values.

use super::pages::{self, PageMeta};
use crate::domain::properties::{self as props, convert_value, display_value, is_computed, validate_value, PROPERTY_TYPES};
use crate::domain::query::{self, PropDef, RecordData};
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PropertyInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ptype: String,
    pub config: Value,
    pub position: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ViewInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub vtype: String,
    pub config: Value,
    pub position: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub properties: Vec<PropertyInfo>,
    pub views: Vec<ViewInfo>,
    pub templates: Vec<PageMeta>,
    pub mcp_access: String,
}

/// Default structure for a new database: Name (title), Status, Date and a Table view.
pub fn init(conn: &Connection, db_id: &str) -> Result<()> {
    conn.execute("INSERT INTO databases(id) VALUES (?1)", [db_id])?;
    let title_id = new_id();
    conn.execute("INSERT INTO database_properties(id, database_id, name, type, config, position) VALUES (?1,?2,'Name','title','{}',0)", params![title_id, db_id])?;
    conn.execute(
        "INSERT INTO database_properties(id, database_id, name, type, config, position) VALUES (?1,?2,'Status','status',?3,1)",
        params![new_id(), db_id, props::default_status_config().to_string()],
    )?;
    conn.execute("INSERT INTO database_properties(id, database_id, name, type, config, position) VALUES (?1,?2,'Date','date','{}',2)", params![new_id(), db_id])?;
    conn.execute("INSERT INTO database_views(id, database_id, name, type, config, position) VALUES (?1,?2,'Table','table','{}',0)", params![new_id(), db_id])?;
    Ok(())
}

fn ensure_db(conn: &Connection, db_id: &str) -> Result<PageMeta> {
    let m = pages::get_meta(conn, db_id)?;
    if m.kind != "database" {
        return invalid("not a database");
    }
    Ok(m)
}

pub fn properties(conn: &Connection, db_id: &str) -> Result<Vec<PropertyInfo>> {
    let mut stmt = conn.prepare("SELECT id, name, type, config, position FROM database_properties WHERE database_id=?1 ORDER BY position")?;
    let rows = stmt.query_map([db_id], |r| {
        Ok(PropertyInfo {
            id: r.get(0)?,
            name: r.get(1)?,
            ptype: r.get(2)?,
            config: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or(json!({})),
            position: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn property(conn: &Connection, prop_id: &str) -> Result<(String, PropertyInfo)> {
    conn.query_row("SELECT database_id, id, name, type, config, position FROM database_properties WHERE id=?1", [prop_id], |r| {
        Ok((
            r.get(0)?,
            PropertyInfo {
                id: r.get(1)?,
                name: r.get(2)?,
                ptype: r.get(3)?,
                config: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or(json!({})),
                position: r.get(5)?,
            },
        ))
    })
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("property {prop_id}")))
}

pub fn views(conn: &Connection, db_id: &str) -> Result<Vec<ViewInfo>> {
    let mut stmt = conn.prepare("SELECT id, name, type, config, position FROM database_views WHERE database_id=?1 ORDER BY position")?;
    let rows = stmt.query_map([db_id], |r| {
        Ok(ViewInfo {
            id: r.get(0)?,
            name: r.get(1)?,
            vtype: r.get(2)?,
            config: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or(json!({})),
            position: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn get(conn: &Connection, db_id: &str) -> Result<DatabaseInfo> {
    let m = ensure_db(conn, db_id)?;
    let mcp_access = conn.query_row("SELECT mcp_access FROM databases WHERE id=?1", [db_id], |r| r.get(0))?;
    let templates = pages::query_metas(
        conn,
        "JOIN database_records r ON r.id = p.id WHERE r.database_id=?1 AND p.is_template=1 AND p.deleted_at IS NULL ORDER BY r.position",
        [db_id],
    )?;
    Ok(DatabaseInfo { id: m.id, title: m.title, icon: m.icon, properties: properties(conn, db_id)?, views: views(conn, db_id)?, templates, mcp_access })
}

pub fn set_mcp_access(conn: &Connection, db_id: &str, access: &str) -> Result<()> {
    ensure_db(conn, db_id)?;
    if !matches!(access, "inherit" | "deny") {
        return invalid("access must be inherit or deny");
    }
    conn.execute("UPDATE databases SET mcp_access=?2 WHERE id=?1", params![db_id, access])?;
    Ok(())
}

// ---------- properties ----------

pub fn add_property(conn: &Connection, db_id: &str, name: &str, ptype: &str) -> Result<PropertyInfo> {
    ensure_db(conn, db_id)?;
    if !PROPERTY_TYPES.contains(&ptype) || ptype == "title" {
        return invalid(format!("unsupported property type {ptype}"));
    }
    let config = match ptype {
        "status" => props::default_status_config(),
        "select" | "multi_select" => json!({ "options": [] }),
        _ => json!({}),
    };
    let pos: f64 = conn.query_row("SELECT COALESCE(MAX(position)+1, 0) FROM database_properties WHERE database_id=?1", [db_id], |r| r.get(0))?;
    let id = new_id();
    let name = if name.trim().is_empty() { "Property" } else { name.trim() };
    conn.execute(
        "INSERT INTO database_properties(id, database_id, name, type, config, position) VALUES (?1,?2,?3,?4,?5,?6)",
        params![id, db_id, name, ptype, config.to_string(), pos],
    )?;
    Ok(property(conn, &id)?.1)
}

pub fn update_property(conn: &Connection, prop_id: &str, patch: &Value) -> Result<PropertyInfo> {
    let (db_id, p) = property(conn, prop_id)?;
    if let Some(name) = patch.get("name").and_then(|n| n.as_str()) {
        if name.trim().is_empty() {
            return invalid("property name is required");
        }
        conn.execute("UPDATE database_properties SET name=?2 WHERE id=?1", params![prop_id, name.trim()])?;
    }
    if let Some(cfg) = patch.get("config") {
        conn.execute("UPDATE database_properties SET config=?2 WHERE id=?1", params![prop_id, cfg.to_string()])?;
        // Drop values that point at options that no longer exist.
        if matches!(p.ptype.as_str(), "select" | "status" | "multi_select") {
            let valid: Vec<String> = props::options(cfg).iter().filter_map(|o| o["id"].as_str().map(String::from)).collect();
            let vals: Vec<(String, String)> = conn
                .prepare("SELECT record_id, value FROM record_values WHERE property_id=?1")?
                .query_map([prop_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?;
            for (rid, raw) in vals {
                let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
                let next = match &v {
                    Value::String(s) if !valid.contains(s) => Some(Value::Null),
                    Value::Array(a) => {
                        let kept: Vec<Value> = a.iter().filter(|x| x.as_str().is_some_and(|s| valid.iter().any(|v| v == s))).cloned().collect();
                        (kept.len() != a.len()).then(|| if kept.is_empty() { Value::Null } else { Value::Array(kept) })
                    }
                    _ => None,
                };
                match next {
                    Some(Value::Null) => conn.execute("DELETE FROM record_values WHERE record_id=?1 AND property_id=?2", params![rid, prop_id])?,
                    Some(nv) => conn.execute("UPDATE record_values SET value=?3 WHERE record_id=?1 AND property_id=?2", params![rid, prop_id, nv.to_string()])?,
                    None => 0,
                };
            }
        }
    }
    if let Some(pos) = patch.get("position").and_then(|p| p.as_f64()) {
        conn.execute("UPDATE database_properties SET position=?2 WHERE id=?1", params![prop_id, pos])?;
    }
    let _ = db_id;
    Ok(property(conn, prop_id)?.1)
}

/// Adds an option to a select/status/multi-select property and returns its id.
pub fn add_option(conn: &Connection, prop_id: &str, name: &str) -> Result<Value> {
    let (_, p) = property(conn, prop_id)?;
    if !matches!(p.ptype.as_str(), "select" | "status" | "multi_select") {
        return invalid("property has no options");
    }
    let mut cfg = p.config.clone();
    let id = props::ensure_option(&mut cfg, name.trim());
    conn.execute("UPDATE database_properties SET config=?2 WHERE id=?1", params![prop_id, cfg.to_string()])?;
    Ok(json!({ "optionId": id, "config": cfg }))
}

pub fn delete_property(conn: &Connection, prop_id: &str) -> Result<()> {
    let (_, p) = property(conn, prop_id)?;
    if p.ptype == "title" {
        return invalid("the title property cannot be deleted");
    }
    conn.execute("DELETE FROM database_properties WHERE id=?1", [prop_id])?;
    Ok(())
}

/// Previews (apply=false) or performs (apply=true) a property type change, reporting values that would be lost.
pub fn change_property_type(conn: &Connection, prop_id: &str, new_type: &str, apply: bool) -> Result<Value> {
    let (db_id, p) = property(conn, prop_id)?;
    if is_computed(&p.ptype) {
        return invalid("this property type cannot be changed");
    }
    if !PROPERTY_TYPES.contains(&new_type) || new_type == "title" {
        return invalid(format!("unsupported property type {new_type}"));
    }
    let mut new_config = match new_type {
        "select" | "multi_select" | "status" if matches!(p.ptype.as_str(), "select" | "multi_select" | "status") => p.config.clone(),
        "select" | "multi_select" | "status" => json!({ "options": [] }),
        _ => json!({}),
    };
    let vals: Vec<(String, String)> = conn
        .prepare("SELECT record_id, value FROM record_values WHERE property_id=?1")?
        .query_map([prop_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut converted = vec![];
    let mut lost = 0;
    let mut lost_samples = vec![];
    for (rid, raw) in &vals {
        let v: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
        let c = if is_computed(new_type) {
            props::Conversion { value: None, lost: true }
        } else {
            convert_value(&p.ptype, &p.config, new_type, &mut new_config, &v)
        };
        if c.lost {
            lost += 1;
            if lost_samples.len() < 5 {
                lost_samples.push(display_value(&p.ptype, &p.config, &v));
            }
        }
        converted.push((rid.clone(), c.value));
    }
    if apply {
        let tx = crate::infrastructure::database::begin(conn)?;
        tx.execute("UPDATE database_properties SET type=?2, config=?3 WHERE id=?1", params![prop_id, new_type, new_config.to_string()])?;
        for (rid, v) in converted {
            match v {
                Some(v) => tx.execute("UPDATE record_values SET value=?3 WHERE record_id=?1 AND property_id=?2", params![rid, prop_id, v.to_string()])?,
                None => tx.execute("DELETE FROM record_values WHERE record_id=?1 AND property_id=?2", params![rid, prop_id])?,
            };
        }
        tx.commit()?;
    }
    let _ = db_id;
    Ok(json!({ "total": vals.len(), "lost": lost, "lostSamples": lost_samples, "applied": apply }))
}

// ---------- views ----------

pub fn add_view(conn: &Connection, db_id: &str, name: &str, vtype: &str) -> Result<ViewInfo> {
    ensure_db(conn, db_id)?;
    if !matches!(vtype, "table" | "board" | "calendar" | "list") {
        return invalid("view type must be table, board, calendar or list");
    }
    let ps = properties(conn, db_id)?;
    let config = match vtype {
        "board" => json!({ "groupBy": ps.iter().find(|p| matches!(p.ptype.as_str(), "status" | "select")).map(|p| p.id.clone()) }),
        "calendar" => json!({ "dateProperty": ps.iter().find(|p| p.ptype == "date").map(|p| p.id.clone()) }),
        _ => json!({}),
    };
    let pos: f64 = conn.query_row("SELECT COALESCE(MAX(position)+1, 0) FROM database_views WHERE database_id=?1", [db_id], |r| r.get(0))?;
    let id = new_id();
    conn.execute(
        "INSERT INTO database_views(id, database_id, name, type, config, position) VALUES (?1,?2,?3,?4,?5,?6)",
        params![id, db_id, if name.trim().is_empty() { vtype } else { name.trim() }, vtype, config.to_string(), pos],
    )?;
    Ok(views(conn, db_id)?.into_iter().find(|v| v.id == id).unwrap())
}

pub fn update_view(conn: &Connection, view_id: &str, patch: &Value) -> Result<ViewInfo> {
    let db_id: String = conn
        .query_row("SELECT database_id FROM database_views WHERE id=?1", [view_id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::NotFound("view".into()))?;
    if let Some(n) = patch.get("name").and_then(|n| n.as_str()) {
        conn.execute("UPDATE database_views SET name=?2 WHERE id=?1", params![view_id, n.trim()])?;
    }
    if let Some(t) = patch.get("type").and_then(|n| n.as_str()) {
        if !matches!(t, "table" | "board" | "calendar" | "list") {
            return invalid("unknown view type");
        }
        conn.execute("UPDATE database_views SET type=?2 WHERE id=?1", params![view_id, t])?;
    }
    if let Some(c) = patch.get("config") {
        if !c.is_object() {
            return invalid("view config must be an object");
        }
        conn.execute("UPDATE database_views SET config=?2 WHERE id=?1", params![view_id, c.to_string()])?;
    }
    Ok(views(conn, &db_id)?.into_iter().find(|v| v.id == view_id).unwrap())
}

pub fn delete_view(conn: &Connection, view_id: &str) -> Result<()> {
    let db_id: String = conn.query_row("SELECT database_id FROM database_views WHERE id=?1", [view_id], |r| r.get(0))?;
    if views(conn, &db_id)?.len() <= 1 {
        return invalid("a database needs at least one view");
    }
    conn.execute("DELETE FROM database_views WHERE id=?1", [view_id])?;
    Ok(())
}

// ---------- records ----------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordOut {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub values: Map<String, Value>,
}

impl From<RecordData> for RecordOut {
    fn from(r: RecordData) -> Self {
        RecordOut { id: r.id, title: r.title, icon: r.icon, created_at: r.created_at, updated_at: r.updated_at, values: r.values }
    }
}

fn local_date(ts: &str, tz: &chrono_tz::Tz) -> String {
    chrono::DateTime::parse_from_rfc3339(ts).map(|d| d.with_timezone(tz).format("%Y-%m-%d").to_string()).unwrap_or_default()
}

fn load_records(conn: &Connection, db_id: &str, templates: bool, only: Option<&str>) -> Result<Vec<RecordData>> {
    let tz = super::settings::user_tz(conn);
    let mut sql = String::from(
        "SELECT r.id, r.position, p.title, p.icon, p.created_at, p.updated_at FROM database_records r JOIN pages p ON p.id = r.id
         WHERE r.database_id=?1 AND p.deleted_at IS NULL AND p.is_template=?2",
    );
    if only.is_some() {
        sql.push_str(" AND r.id = ?3");
    }
    let mut stmt = conn.prepare(&sql)?;
    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<RecordData> {
        let created: String = r.get(4)?;
        let updated: String = r.get(5)?;
        Ok(RecordData {
            id: r.get(0)?,
            position: r.get(1)?,
            title: r.get(2)?,
            icon: r.get(3)?,
            created_date: local_date(&created, &tz),
            updated_date: local_date(&updated, &tz),
            created_at: created,
            updated_at: updated,
            values: Map::new(),
        })
    };
    let mut recs: Vec<RecordData> = match only {
        Some(id) => stmt.query_map(params![db_id, templates as i64, id], map_row)?.collect::<std::result::Result<_, _>>()?,
        None => stmt.query_map(params![db_id, templates as i64], map_row)?.collect::<std::result::Result<_, _>>()?,
    };
    let index: HashMap<String, usize> = recs.iter().enumerate().map(|(i, r)| (r.id.clone(), i)).collect();
    let mut vstmt = conn.prepare(
        "SELECT v.record_id, v.property_id, v.value FROM record_values v JOIN database_records r ON r.id = v.record_id WHERE r.database_id=?1",
    )?;
    let vals = vstmt.query_map([db_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
    for v in vals {
        let (rid, pid, raw) = v?;
        if let Some(&i) = index.get(&rid) {
            recs[i].values.insert(pid, serde_json::from_str(&raw).unwrap_or(Value::Null));
        }
    }
    Ok(recs)
}

fn prop_defs(ps: &[PropertyInfo]) -> HashMap<String, PropDef> {
    ps.iter()
        .map(|p| (p.id.clone(), PropDef { id: p.id.clone(), name: p.name.clone(), ptype: p.ptype.clone(), config: p.config.clone() }))
        .collect()
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct QuerySpec {
    pub view_id: Option<String>,
    pub filter: Option<Value>,
    pub sorts: Option<Vec<Value>>,
    /// Quick text filter across title and text values.
    pub search: Option<String>,
    pub offset: usize,
    pub limit: Option<usize>,
    pub templates: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub total: usize,
    pub offset: usize,
    pub records: Vec<RecordOut>,
}

pub fn query(conn: &Connection, db_id: &str, spec: &QuerySpec) -> Result<QueryResult> {
    ensure_db(conn, db_id)?;
    let ps = properties(conn, db_id)?;
    let defs = prop_defs(&ps);
    let view_cfg = match &spec.view_id {
        Some(v) => views(conn, db_id)?.into_iter().find(|x| &x.id == v).map(|x| x.config).unwrap_or(json!({})),
        None => json!({}),
    };
    let filter = spec.filter.clone().or_else(|| view_cfg.get("filter").cloned()).unwrap_or(Value::Null);
    let sorts = spec.sorts.clone().or_else(|| view_cfg.get("sorts").and_then(|s| s.as_array()).cloned()).unwrap_or_default();
    let mut recs = load_records(conn, db_id, spec.templates, None)?;
    if !filter.is_null() {
        recs.retain(|r| query::matches(&filter, r, &defs));
    }
    if let Some(q) = spec.search.as_deref().map(str::to_lowercase).filter(|q| !q.is_empty()) {
        recs.retain(|r| {
            r.title.to_lowercase().contains(&q)
                || ps.iter().any(|p| display_value(&p.ptype, &p.config, &r.value_of(&defs[&p.id])).to_lowercase().contains(&q))
        });
    }
    query::sort(&mut recs, &sorts, &defs);
    let total = recs.len();
    let limit = spec.limit.unwrap_or(500).clamp(1, 5000);
    let records = recs.into_iter().skip(spec.offset).take(limit).map(RecordOut::from).collect();
    Ok(QueryResult { total, offset: spec.offset, records })
}

pub fn get_record(conn: &Connection, record_id: &str) -> Result<RecordOut> {
    let (db_id, is_tpl): (String, i64) = conn
        .query_row("SELECT r.database_id, p.is_template FROM database_records r JOIN pages p ON p.id=r.id WHERE r.id=?1", [record_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("record {record_id}")))?;
    load_records(conn, &db_id, is_tpl != 0, Some(record_id))?
        .into_iter()
        .next()
        .map(RecordOut::from)
        .ok_or_else(|| AppError::NotFound(format!("record {record_id}")))
}

pub fn database_of_record(conn: &Connection, record_id: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT database_id FROM database_records WHERE id=?1", [record_id], |r| r.get(0)).optional()?)
}

pub fn record_values_text(conn: &Connection, record_id: &str) -> Result<String> {
    let Some(db_id) = database_of_record(conn, record_id)? else { return Ok(String::new()) };
    let ps = properties(conn, &db_id)?;
    let mut stmt = conn.prepare("SELECT property_id, value FROM record_values WHERE record_id=?1")?;
    let vals: Vec<(String, String)> = stmt.query_map([record_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
    Ok(vals
        .iter()
        .filter_map(|(pid, raw)| {
            let p = ps.iter().find(|p| &p.id == pid)?;
            Some(display_value(&p.ptype, &p.config, &serde_json::from_str(raw).ok()?))
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · "))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateRecord {
    pub title: String,
    pub values: Map<String, Value>,
    pub template_id: Option<String>,
    pub is_template: bool,
}

pub fn create_record(conn: &Connection, db_id: &str, c: CreateRecord) -> Result<RecordOut> {
    let db = ensure_db(conn, db_id)?;
    let id = match &c.template_id {
        Some(tpl) => {
            let t = pages::get_meta(conn, tpl)?;
            let title = if c.title.trim().is_empty() { t.title.clone() } else { c.title.clone() };
            pages::duplicate_subtree(conn, tpl, Some(db_id), None, Some(&title), Some(c.is_template))?
        }
        None => {
            let id = new_id();
            let ts = now();
            let tx = crate::infrastructure::database::begin(conn)?;
            let pos: f64 = tx.query_row("SELECT COALESCE(MAX(position)+1, 0) FROM database_records WHERE database_id=?1", [db_id], |r| r.get(0))?;
            tx.execute(
                "INSERT INTO pages(id, section_id, parent_id, kind, title, position, is_template, created_at, updated_at) VALUES (?1,?2,?3,'record',?4,?5,?6,?7,?7)",
                params![id, db.section_id, db_id, c.title.trim(), pos, c.is_template as i64, ts],
            )?;
            tx.execute("INSERT INTO database_records(id, database_id, position) VALUES (?1,?2,?3)", params![id, db_id, pos])?;
            tx.commit()?;
            id
        }
    };
    for (pid, v) in &c.values {
        set_value(conn, &id, pid, v)?;
    }
    pages::index(conn, &id)?;
    get_record(conn, &id)
}

pub fn set_value(conn: &Connection, record_id: &str, prop_id: &str, value: &Value) -> Result<RecordOut> {
    let (db_id, p) = property(conn, prop_id)?;
    if database_of_record(conn, record_id)?.as_deref() != Some(db_id.as_str()) {
        return invalid("property belongs to another database");
    }
    let ts = now();
    match p.ptype.as_str() {
        "title" => {
            conn.execute("UPDATE pages SET title=?2, updated_at=?3 WHERE id=?1", params![record_id, value.as_str().unwrap_or("").trim(), ts])?;
        }
        t if is_computed(t) => return invalid("this property is computed automatically"),
        t => {
            match validate_value(t, &p.config, value)? {
                Some(v) => conn.execute(
                    "INSERT INTO record_values(record_id, property_id, value) VALUES (?1,?2,?3)
                     ON CONFLICT(record_id, property_id) DO UPDATE SET value=excluded.value",
                    params![record_id, prop_id, v.to_string()],
                )?,
                None => conn.execute("DELETE FROM record_values WHERE record_id=?1 AND property_id=?2", params![record_id, prop_id])?,
            };
            conn.execute("UPDATE pages SET updated_at=?2 WHERE id=?1", params![record_id, ts])?;
        }
    }
    pages::index(conn, record_id)?;
    get_record(conn, record_id)
}

pub fn move_record(conn: &Connection, record_id: &str, before_id: Option<&str>) -> Result<()> {
    let db_id = database_of_record(conn, record_id)?.ok_or_else(|| AppError::NotFound("record".into()))?;
    let mut ids: Vec<String> = conn
        .prepare("SELECT id FROM database_records WHERE database_id=?1 AND id<>?2 ORDER BY position")?
        .query_map(params![db_id, record_id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let at = before_id.and_then(|b| ids.iter().position(|i| i == b)).unwrap_or(ids.len());
    ids.insert(at, record_id.to_string());
    let tx = crate::infrastructure::database::begin(conn)?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute("UPDATE database_records SET position=?2 WHERE id=?1", params![id, i as f64])?;
    }
    tx.commit()?;
    Ok(())
}

// ---------- CSV / JSON ----------

pub fn to_csv(conn: &Connection, db_id: &str) -> Result<String> {
    let ps = properties(conn, db_id)?;
    let defs = prop_defs(&ps);
    let recs = query(conn, db_id, &QuerySpec { limit: Some(5000), ..Default::default() })?;
    let mut rows = vec![ps.iter().map(|p| p.name.clone()).collect::<Vec<_>>()];
    let all = load_records(conn, db_id, false, None)?;
    let mut by_id: HashMap<String, RecordData> = all.into_iter().map(|r| (r.id.clone(), r)).collect();
    for r in recs.records {
        if let Some(rd) = by_id.remove(&r.id) {
            rows.push(ps.iter().map(|p| display_value(&p.ptype, &p.config, &rd.value_of(&defs[&p.id]))).collect());
        }
    }
    Ok(crate::infrastructure::export::write_csv(&rows))
}

pub fn to_json(conn: &Connection, db_id: &str) -> Result<Value> {
    let info = get(conn, db_id)?;
    let recs = query(conn, db_id, &QuerySpec { limit: Some(5000), ..Default::default() })?;
    Ok(json!({
        "format": "personal-workspace-database",
        "formatVersion": 1,
        "database": { "id": info.id, "title": info.title },
        "properties": info.properties,
        "views": info.views,
        "records": recs.records,
    }))
}

/// Creates a database from CSV text. The first row holds column names; the first column becomes the title.
pub fn import_csv(conn: &Connection, csv: &str, title: &str, section_id: Option<String>, parent_id: Option<String>) -> Result<PageMeta> {
    let rows = crate::infrastructure::export::parse_csv(csv.trim_start_matches('\u{feff}'));
    let Some(header) = rows.first() else { return invalid("the CSV file is empty") };
    let db = pages::create(conn, pages::CreatePage { kind: "database".into(), title: title.into(), section_id, parent_id, ..Default::default() })?;
    // Replace default properties with the CSV columns.
    let tx = crate::infrastructure::database::begin(conn)?;
    tx.execute("DELETE FROM database_properties WHERE database_id=?1 AND type<>'title'", [&db.id])?;
    tx.execute("UPDATE database_properties SET name=?2 WHERE database_id=?1 AND type='title'", params![db.id, header[0].trim()])?;
    tx.commit()?;
    let data = &rows[1..];
    let mut col_props: Vec<Option<PropertyInfo>> = vec![None];
    for (ci, name) in header.iter().enumerate().skip(1) {
        let col: Vec<&str> = data.iter().filter_map(|r| r.get(ci)).map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        let ptype = if !col.is_empty() && col.iter().all(|v| v.parse::<f64>().is_ok()) {
            "number"
        } else if !col.is_empty() && col.iter().all(|v| props::is_valid_date(v) && v.len() == 10) {
            "date"
        } else if !col.is_empty() && col.iter().all(|v| matches!(v.to_lowercase().as_str(), "true" | "false" | "yes" | "no")) {
            "checkbox"
        } else {
            "text"
        };
        col_props.push(Some(add_property(conn, &db.id, name, ptype)?));
    }
    for row in data {
        let mut values = Map::new();
        for (ci, cell) in row.iter().enumerate().skip(1) {
            let Some(Some(p)) = col_props.get(ci) else { continue };
            let cell = cell.trim();
            if cell.is_empty() {
                continue;
            }
            let v = match p.ptype.as_str() {
                "number" => json!(cell.parse::<f64>().unwrap_or(0.0)),
                "date" => json!({ "start": cell }),
                "checkbox" => json!(matches!(cell.to_lowercase().as_str(), "true" | "yes")),
                _ => json!(cell),
            };
            values.insert(p.id.clone(), v);
        }
        create_record(conn, &db.id, CreateRecord { title: row.first().cloned().unwrap_or_default(), values, ..Default::default() })?;
    }
    pages::get_meta(conn, &db.id)
}

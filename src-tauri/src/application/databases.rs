//! Databases: properties, views, records and record values.

use super::pages::{self, PageMeta};
use crate::domain::properties::{self as props, convert_value, display_value, is_computed, validate_value, PROPERTY_TYPES};
use crate::domain::formula::{self, FV};
use crate::domain::query::{self, PropDef, RecordData};
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[derive(Serialize, Clone, Debug)]
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
        validate_config(conn, &db_id, &p, cfg)?;
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
    Ok(property(conn, prop_id)?.1)
}

pub const ROLLUP_FUNCTIONS: &[&str] = &["count", "count_values", "sum", "average", "min", "max", "percent_checked", "earliest", "latest", "show_original"];

/// Checks formula syntax and cycles, relation targets and rollup wiring before a config is saved.
fn validate_config(conn: &Connection, db_id: &str, p: &PropertyInfo, cfg: &Value) -> Result<()> {
    match p.ptype.as_str() {
        "formula" => {
            let expr = cfg.get("expression").and_then(|e| e.as_str()).unwrap_or("");
            formula::parse(expr).map_err(|e| AppError::Invalid(format!("Formula error: {e}")))?;
            // Build name → referenced names for every formula in the database, with this one replaced.
            let ps = properties(conn, db_id)?;
            let mut graph: HashMap<String, Vec<String>> = HashMap::new();
            for q in ps.iter().filter(|q| q.ptype == "formula") {
                let src = if q.id == p.id { expr.to_string() } else { q.config.get("expression").and_then(|e| e.as_str()).unwrap_or("").to_string() };
                let mut refs = vec![];
                if let Ok(e) = formula::parse(&src) {
                    formula::references(&e, &mut refs);
                }
                graph.insert(q.name.clone(), refs);
            }
            fn visit(n: &str, g: &HashMap<String, Vec<String>>, path: &mut Vec<String>) -> Option<Vec<String>> {
                if let Some(i) = path.iter().position(|x| x == n) {
                    let mut cyc = path[i..].to_vec();
                    cyc.push(n.to_string());
                    return Some(cyc);
                }
                let refs = g.get(n)?;
                path.push(n.to_string());
                for r in refs {
                    if let Some(c) = visit(r, g, path) {
                        return Some(c);
                    }
                }
                path.pop();
                None
            }
            if let Some(cyc) = visit(&p.name, &graph, &mut vec![]) {
                return invalid(format!("Circular formula: {}", cyc.join(" → ")));
            }
        }
        "relation" => {
            if let Some(t) = cfg.get("databaseId").and_then(|d| d.as_str()).filter(|d| !d.is_empty()) {
                ensure_db(conn, t)?;
            }
        }
        "rollup" => {
            let rel = cfg.get("relationPropertyId").and_then(|v| v.as_str()).unwrap_or("");
            let func = cfg.get("function").and_then(|v| v.as_str()).unwrap_or("count");
            if !ROLLUP_FUNCTIONS.contains(&func) {
                return invalid("unknown rollup function");
            }
            if !rel.is_empty() {
                let (rdb, rp) = property(conn, rel)?;
                if rdb != db_id || rp.ptype != "relation" {
                    return invalid("a rollup must use a relation property of this database");
                }
                if let Some(tp) = cfg.get("targetPropertyId").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
                    let (tdb, _) = property(conn, tp)?;
                    if Some(tdb.as_str()) != rp.config.get("databaseId").and_then(|d| d.as_str()) {
                        return invalid("the rolled-up property must belong to the related database");
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ---------- derived values: relations, rollups, formulas ----------

fn titles_for(conn: &Connection, ids: &[String]) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    let mut stmt = conn.prepare("SELECT title FROM pages WHERE id=?1 AND deleted_at IS NULL")?;
    for id in ids {
        if out.contains_key(id) {
            continue;
        }
        if let Some(t) = stmt.query_row([id], |r| r.get::<_, String>(0)).optional()? {
            out.insert(id.clone(), t);
        }
    }
    Ok(out)
}

fn json_to_fv(v: &Value) -> FV {
    match v {
        Value::Number(n) => FV::Num(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => FV::Text(s.clone()),
        Value::Bool(b) => FV::Bool(*b),
        Value::Array(a) => FV::List(a.iter().map(json_to_fv).collect()),
        Value::Object(o) => match o.get("start").and_then(|s| s.as_str()).and_then(crate::domain::growth::date) {
            Some(d) => FV::Date(d),
            None => FV::Null,
        },
        Value::Null => FV::Null,
    }
}

/// A property value as a formula value (select → option name, relation → titles, …).
fn prop_fv(p: &PropertyInfo, rec: &RecordData, titles: &HashMap<String, String>) -> FV {
    let date = |s: &str| crate::domain::growth::date(s).map(FV::Date).unwrap_or(FV::Null);
    match p.ptype.as_str() {
        "title" => FV::Text(rec.title.clone()),
        "created_time" => date(&rec.created_date),
        "last_edited_time" => date(&rec.updated_date),
        t => {
            let v = rec.values.get(&p.id).cloned().unwrap_or(Value::Null);
            match t {
                "checkbox" => FV::Bool(v.as_bool() == Some(true)),
                "select" | "status" => v.as_str().and_then(|id| props::option_name(&p.config, id)).map(FV::Text).unwrap_or(FV::Null),
                "multi_select" => FV::List(v.as_array().into_iter().flatten().filter_map(|id| id.as_str().and_then(|id| props::option_name(&p.config, id))).map(FV::Text).collect()),
                "relation" => FV::List(v.as_array().into_iter().flatten().filter_map(|id| id.as_str().and_then(|id| titles.get(id))).map(|t| FV::Text(t.clone())).collect()),
                "files" => FV::List(v.as_array().into_iter().flatten().filter_map(|f| f.get("name").and_then(|n| n.as_str())).map(|n| FV::Text(n.into())).collect()),
                _ if v.get("error").is_some() => FV::Null,
                _ => json_to_fv(&v),
            }
        }
    }
}

type Materialized = (Vec<PropertyInfo>, HashMap<String, RecordData>, HashMap<String, String>);

/// Fills rollup and formula values into `recs` and returns titles of related records.
/// `depth` bounds rollups that read other databases' rollups (and self-relations).
fn compute_derived(conn: &Connection, ps: &[PropertyInfo], recs: &mut [RecordData], depth: usize, cache: &mut HashMap<String, Materialized>) -> Result<HashMap<String, String>> {
    let mut ids = vec![];
    for p in ps.iter().filter(|p| p.ptype == "relation") {
        for r in recs.iter() {
            ids.extend(r.values.get(&p.id).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|x| x.as_str().map(String::from)));
        }
    }
    let titles = titles_for(conn, &ids)?;

    for p in ps.iter().filter(|p| p.ptype == "rollup") {
        let rel = p.config.get("relationPropertyId").and_then(|v| v.as_str()).and_then(|id| ps.iter().find(|x| x.id == id));
        let func = p.config.get("function").and_then(|v| v.as_str()).unwrap_or("count");
        let Some(rel) = rel else {
            recs.iter_mut().for_each(|r| { r.values.insert(p.id.clone(), json!({ "error": "choose a relation property" })); });
            continue;
        };
        let target_db = rel.config.get("databaseId").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if depth >= 2 {
            recs.iter_mut().for_each(|r| { r.values.insert(p.id.clone(), json!({ "error": "rollups are nested too deeply" })); });
            continue;
        }
        if !cache.contains_key(&target_db) && !target_db.is_empty() {
            let tps = properties(conn, &target_db)?;
            let mut trecs = load_records(conn, &target_db, false, None)?;
            let tt = compute_derived(conn, &tps, &mut trecs, depth + 1, cache)?;
            cache.insert(target_db.clone(), (tps, trecs.into_iter().map(|r| (r.id.clone(), r)).collect(), tt));
        }
        let Some((tps, trecs, ttitles)) = cache.get(&target_db) else { continue };
        let target = p.config.get("targetPropertyId").and_then(|v| v.as_str()).and_then(|id| tps.iter().find(|x| x.id == id));
        for r in recs.iter_mut() {
            let linked: Vec<&RecordData> = r.values.get(&rel.id).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|x| x.as_str().and_then(|id| trecs.get(id))).collect();
            let vals: Vec<FV> = match target {
                Some(t) => linked.iter().map(|lr| prop_fv(t, lr, ttitles)).flat_map(|v| match v { FV::List(l) => l, x => vec![x] }).collect(),
                None => vec![],
            };
            let present: Vec<&FV> = vals.iter().filter(|v| !matches!(v, FV::Null) && v.as_text() != "").collect();
            let nums: Vec<f64> = present.iter().filter_map(|v| match v { FV::Num(n) => Some(*n), _ => None }).collect();
            let dates: Vec<chrono::NaiveDate> = present.iter().filter_map(|v| match v { FV::Date(d) => Some(*d), _ => None }).collect();
            let out = match func {
                "count" => FV::Num(linked.len() as f64),
                "count_values" => FV::Num(present.len() as f64),
                "sum" => FV::Num(nums.iter().sum()),
                "average" if !nums.is_empty() => FV::Num(nums.iter().sum::<f64>() / nums.len() as f64),
                "min" if !nums.is_empty() => FV::Num(nums.iter().cloned().fold(f64::INFINITY, f64::min)),
                "max" if !nums.is_empty() => FV::Num(nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
                "percent_checked" if !vals.is_empty() => FV::Num((vals.iter().filter(|v| **v == FV::Bool(true)).count() as f64 / vals.len() as f64 * 1000.0).round() / 10.0),
                "earliest" => dates.iter().min().map(|d| FV::Date(*d)).unwrap_or(FV::Null),
                "latest" => dates.iter().max().map(|d| FV::Date(*d)).unwrap_or(FV::Null),
                "show_original" => FV::List(present.into_iter().cloned().collect()),
                _ => FV::Null,
            };
            r.values.insert(p.id.clone(), if target.is_none() && func != "count" { json!({ "error": "choose the property to roll up" }) } else { out.to_json() });
        }
    }

    let formulas: Vec<(&PropertyInfo, std::result::Result<formula::Expr, String>)> = ps
        .iter()
        .filter(|p| p.ptype == "formula")
        .map(|p| (p, formula::parse(p.config.get("expression").and_then(|e| e.as_str()).unwrap_or(""))))
        .collect();
    if formulas.is_empty() {
        return Ok(titles);
    }
    let today = crate::domain::growth::date(&super::tasks::today(conn)).unwrap_or_else(|| chrono::Utc::now().date_naive());

    fn eval_prop(
        pid: &str, rec: &RecordData, ps: &[PropertyInfo], formulas: &[(&PropertyInfo, std::result::Result<formula::Expr, String>)],
        titles: &HashMap<String, String>, today: chrono::NaiveDate, stack: &mut Vec<String>, done: &mut HashMap<String, std::result::Result<FV, String>>,
    ) -> std::result::Result<FV, String> {
        if let Some(r) = done.get(pid) {
            return r.clone();
        }
        let (fp, expr) = formulas.iter().find(|(p, _)| p.id == pid).ok_or("unknown formula")?;
        if stack.iter().any(|s| s == pid) {
            let names: Vec<String> = stack.iter().filter_map(|id| ps.iter().find(|p| &p.id == id).map(|p| p.name.clone())).chain([fp.name.clone()]).collect();
            return Err(format!("circular reference: {}", names.join(" → ")));
        }
        let expr = expr.clone()?;
        stack.push(pid.to_string());
        let result = {
            let mut resolve = |name: &str| -> std::result::Result<FV, String> {
                let p = ps.iter().find(|p| p.name == name).or_else(|| ps.iter().find(|p| p.name.eq_ignore_ascii_case(name))).ok_or(format!("no property named “{name}”"))?;
                if p.ptype == "formula" {
                    eval_prop(&p.id, rec, ps, formulas, titles, today, stack, done)
                } else {
                    Ok(prop_fv(p, rec, titles))
                }
            };
            formula::eval(&expr, &mut resolve, today, 0)
        };
        stack.pop();
        done.insert(pid.to_string(), result.clone());
        result
    }

    for r in recs.iter_mut() {
        let mut done = HashMap::new();
        let results: Vec<(String, Value)> = formulas
            .iter()
            .map(|(p, _)| {
                let v = match eval_prop(&p.id, r, ps, &formulas, &titles, today, &mut vec![], &mut done) {
                    Ok(v) => v.to_json(),
                    Err(e) => json!({ "error": e }),
                };
                (p.id.clone(), v)
            })
            .collect();
        for (k, v) in results {
            r.values.insert(k, v);
        }
    }
    Ok(titles)
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

pub const VIEW_TYPES: &[&str] = &["table", "board", "calendar", "list", "timeline", "gallery", "chart"];

pub fn add_view(conn: &Connection, db_id: &str, name: &str, vtype: &str) -> Result<ViewInfo> {
    ensure_db(conn, db_id)?;
    if !VIEW_TYPES.contains(&vtype) {
        return invalid("unknown view type");
    }
    let ps = properties(conn, db_id)?;
    let config = match vtype {
        "board" => json!({ "groupBy": ps.iter().find(|p| matches!(p.ptype.as_str(), "status" | "select")).map(|p| p.id.clone()) }),
        "calendar" | "timeline" => json!({ "dateProperty": ps.iter().find(|p| p.ptype == "date").map(|p| p.id.clone()) }),
        "gallery" => json!({ "coverProperty": ps.iter().find(|p| p.ptype == "files").map(|p| p.id.clone()) }),
        "chart" => json!({ "chartType": "bar", "groupBy": ps.iter().find(|p| matches!(p.ptype.as_str(), "status" | "select")).map(|p| p.id.clone()), "aggregate": "count" }),
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
        if !VIEW_TYPES.contains(&t) {
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
    /// Titles of records referenced by this record's relation properties (deleted ones are absent).
    pub relation_titles: HashMap<String, String>,
}

impl From<RecordData> for RecordOut {
    fn from(r: RecordData) -> Self {
        RecordOut { id: r.id, title: r.title, icon: r.icon, created_at: r.created_at, updated_at: r.updated_at, values: r.values, relation_titles: HashMap::new() }
    }
}

fn to_out(r: RecordData, ps: &[PropertyInfo], titles: &HashMap<String, String>) -> RecordOut {
    let mut rel = HashMap::new();
    for p in ps.iter().filter(|p| p.ptype == "relation") {
        for id in r.values.get(&p.id).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|x| x.as_str()) {
            if let Some(t) = titles.get(id) {
                rel.insert(id.to_string(), t.clone());
            }
        }
    }
    let mut out = RecordOut::from(r);
    out.relation_titles = rel;
    out
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
    // A single record reads only its own values (keeps edits fast in large databases).
    let mut vstmt = conn.prepare(
        "SELECT v.record_id, v.property_id, v.value FROM record_values v JOIN database_records r ON r.id = v.record_id
         WHERE r.database_id=?1 AND (?2 IS NULL OR v.record_id = ?2)",
    )?;
    let vals = vstmt.query_map(params![db_id, only], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
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
    let titles = compute_derived(conn, &ps, &mut recs, 0, &mut HashMap::new())?;
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
    let records = recs.into_iter().skip(spec.offset).take(limit).map(|r| to_out(r, &ps, &titles)).collect();
    Ok(QueryResult { total, offset: spec.offset, records })
}

pub fn get_record(conn: &Connection, record_id: &str) -> Result<RecordOut> {
    let (db_id, is_tpl): (String, i64) = conn
        .query_row("SELECT r.database_id, p.is_template FROM database_records r JOIN pages p ON p.id=r.id WHERE r.id=?1", [record_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("record {record_id}")))?;
    let ps = properties(conn, &db_id)?;
    let mut recs = load_records(conn, &db_id, is_tpl != 0, Some(record_id))?;
    let titles = compute_derived(conn, &ps, &mut recs, 0, &mut HashMap::new())?;
    recs.into_iter().next().map(|r| to_out(r, &ps, &titles)).ok_or_else(|| AppError::NotFound(format!("record {record_id}")))
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
        write_value(conn, &id, pid, v)?;
    }
    pages::index(conn, &id)?;
    if !c.is_template {
        super::automations::dispatch(conn, json!({ "type": "record_created", "databaseId": db_id, "recordId": id }));
    }
    get_record(conn, &id)
}

pub fn set_value(conn: &Connection, record_id: &str, prop_id: &str, value: &Value) -> Result<RecordOut> {
    let db_id = write_value(conn, record_id, prop_id, value)?;
    super::automations::dispatch(conn, json!({ "type": "record_updated", "databaseId": db_id, "recordId": record_id, "propertyId": prop_id }));
    get_record(conn, record_id)
}

/// Validates and stores one value (no automation events). Returns the database id.
fn write_value(conn: &Connection, record_id: &str, prop_id: &str, value: &Value) -> Result<String> {
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
        "relation" => {
            let target = p.config.get("databaseId").and_then(|d| d.as_str()).filter(|d| !d.is_empty())
                .ok_or_else(|| AppError::Invalid("choose the related database for this property first".into()))?;
            let v = validate_value("relation", &p.config, value)?;
            for id in v.iter().flat_map(|v| v.as_array().cloned().unwrap_or_default()) {
                let id = id.as_str().unwrap_or("");
                if database_of_record(conn, id)?.as_deref() != Some(target) {
                    return invalid("a linked record does not belong to the related database");
                }
            }
            match v {
                Some(v) => conn.execute(
                    "INSERT INTO record_values(record_id, property_id, value) VALUES (?1,?2,?3) ON CONFLICT(record_id, property_id) DO UPDATE SET value=excluded.value",
                    params![record_id, prop_id, v.to_string()],
                )?,
                None => conn.execute("DELETE FROM record_values WHERE record_id=?1 AND property_id=?2", params![record_id, prop_id])?,
            };
            conn.execute("UPDATE pages SET updated_at=?2 WHERE id=?1", params![record_id, ts])?;
        }
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
    Ok(db_id)
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

/// Display text of one property of a query result (titles for relations, computed values for formulas).
pub fn record_display(p: &PropertyInfo, r: &RecordOut) -> String {
    match p.ptype.as_str() {
        "title" => r.title.clone(),
        "created_time" => r.created_at.clone(),
        "last_edited_time" => r.updated_at.clone(),
        "relation" => r.values.get(&p.id).and_then(|v| v.as_array()).into_iter().flatten()
            .filter_map(|id| id.as_str().and_then(|id| r.relation_titles.get(id)).cloned())
            .collect::<Vec<_>>().join("; "),
        _ => display_value(&p.ptype, &p.config, r.values.get(&p.id).unwrap_or(&Value::Null)),
    }
}

pub fn to_csv(conn: &Connection, db_id: &str) -> Result<String> {
    let ps = properties(conn, db_id)?;
    let recs = query(conn, db_id, &QuerySpec { limit: Some(5000), ..Default::default() })?;
    let mut rows = vec![ps.iter().map(|p| p.name.clone()).collect::<Vec<_>>()];
    for r in &recs.records {
        rows.push(ps.iter().map(|p| record_display(p, r)).collect());
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

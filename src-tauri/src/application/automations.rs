//! Automations: Trigger → Condition → Action, run inside the app's services.
//!
//! Safety: every event carries an id used as an idempotency key (a rule never runs twice for one event),
//! a per-chain guard stops a rule from re-triggering itself on the same item, nesting is capped, and each
//! run is logged with its outcome. Actions of one run are atomic: on error they roll back together.

use super::{databases, settings, tasks, templates};
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use chrono::{Datelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::cell::RefCell;

pub const TRIGGERS: &[&str] = &["record_created", "record_updated", "task_created", "task_status_changed", "schedule"];
pub const ACTIONS: &[&str] = &["set_property", "create_task", "set_task_status", "create_page_from_template", "notify"];
const MAX_DEPTH: usize = 3;

thread_local! {
    /// (automation id, entity id) pairs in the current chain of automation-caused changes.
    static CHAIN: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

// ---------- CRUD ----------

fn row_json(conn: &Connection, id: &str) -> Result<Value> {
    let mut v = conn
        .query_row("SELECT id, name, enabled, config, created_at, updated_at FROM automations WHERE id=?1", [id], |r| {
            Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "enabled": r.get::<_, i64>(2)? != 0,
                       "config": serde_json::from_str::<Value>(&r.get::<_, String>(3)?).unwrap_or(json!({})),
                       "createdAt": r.get::<_, String>(4)?, "updatedAt": r.get::<_, Option<String>>(5)? }))
        })
        .optional()?
        .ok_or_else(|| AppError::NotFound("automation".into()))?;
    v["lastRun"] = conn
        .query_row("SELECT status, detail, at FROM automation_runs WHERE automation_id=?1 ORDER BY at DESC LIMIT 1", [id], |r| {
            Ok(json!({ "status": r.get::<_, String>(0)?, "detail": r.get::<_, Option<String>>(1)?, "at": r.get::<_, String>(2)? }))
        })
        .optional()?
        .unwrap_or(Value::Null);
    Ok(v)
}

pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    let ids: Vec<String> = conn.prepare("SELECT id FROM automations ORDER BY created_at")?.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    ids.iter().map(|id| row_json(conn, id)).collect()
}

fn validate(conn: &Connection, cfg: &Value) -> Result<()> {
    let trig = cfg.pointer("/trigger/type").and_then(|t| t.as_str()).unwrap_or("");
    if !TRIGGERS.contains(&trig) {
        return invalid("choose a trigger");
    }
    if trig.starts_with("record_") {
        let db = cfg.pointer("/trigger/databaseId").and_then(|t| t.as_str()).unwrap_or("");
        databases::get(conn, db).map_err(|_| AppError::Invalid("choose the database this rule watches".into()))?;
    }
    if trig == "schedule" {
        let t = cfg.pointer("/trigger/time").and_then(|t| t.as_str()).unwrap_or("");
        if chrono::NaiveTime::parse_from_str(t, "%H:%M").is_err() {
            return invalid("schedule time must be HH:MM");
        }
    }
    let actions = cfg.get("actions").and_then(|a| a.as_array()).ok_or_else(|| AppError::Invalid("add at least one action".into()))?;
    if actions.is_empty() || actions.len() > 10 {
        return invalid("a rule needs 1–10 actions");
    }
    for a in actions {
        let t = a.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if !ACTIONS.contains(&t) {
            return invalid(format!("unknown action {t}"));
        }
        if t == "set_property" && !trig.starts_with("record_") {
            return invalid("“set property” only works with database triggers");
        }
        if t == "set_task_status" && !trig.starts_with("task_") && cfg.pointer("/trigger/forEach").is_none() {
            return invalid("“set task status” needs a task trigger");
        }
    }
    Ok(())
}

pub fn create(conn: &Connection, name: &str, cfg: &Value) -> Result<Value> {
    validate(conn, cfg)?;
    let id = new_id();
    let ts = now();
    conn.execute(
        "INSERT INTO automations(id, name, enabled, config, created_at, updated_at) VALUES (?1,?2,1,?3,?4,?4)",
        params![id, if name.trim().is_empty() { "Automation" } else { name.trim() }, cfg.to_string(), ts],
    )?;
    row_json(conn, &id)
}

pub fn update(conn: &Connection, id: &str, patch: &Value) -> Result<Value> {
    row_json(conn, id)?;
    if let Some(n) = patch.get("name").and_then(|v| v.as_str()) {
        conn.execute("UPDATE automations SET name=?2 WHERE id=?1", params![id, n.trim()])?;
    }
    if let Some(e) = patch.get("enabled").and_then(|v| v.as_bool()) {
        conn.execute("UPDATE automations SET enabled=?2 WHERE id=?1", params![id, e as i64])?;
    }
    if let Some(cfg) = patch.get("config") {
        validate(conn, cfg)?;
        conn.execute("UPDATE automations SET config=?2 WHERE id=?1", params![id, cfg.to_string()])?;
    }
    conn.execute("UPDATE automations SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    row_json(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM automations WHERE id=?1", [id])?;
    Ok(())
}

pub fn runs(conn: &Connection, id: Option<&str>, limit: usize) -> Result<Vec<Value>> {
    let rows = conn
        .prepare(
            "SELECT r.id, r.automation_id, a.name, r.status, r.detail, r.at FROM automation_runs r JOIN automations a ON a.id=r.automation_id
             WHERE (?1 IS NULL OR r.automation_id=?1) ORDER BY r.at DESC LIMIT ?2",
        )?
        .query_map(params![id, limit as i64], |r| {
            Ok(json!({ "id": r.get::<_, String>(0)?, "automationId": r.get::<_, String>(1)?, "automationName": r.get::<_, String>(2)?,
                       "status": r.get::<_, String>(3)?, "detail": r.get::<_, Option<String>>(4)?, "at": r.get::<_, String>(5)? }))
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

// ---------- engine ----------

/// Entry point used by services after a change is saved. Never fails the caller: problems are logged as runs.
pub fn dispatch(conn: &Connection, event: Value) {
    if let Err(e) = dispatch_inner(conn, event) {
        eprintln!("automation dispatch failed: {e}");
    }
}

fn entity_of(event: &Value) -> String {
    event.get("recordId").or_else(|| event.get("taskId")).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn dispatch_inner(conn: &Connection, mut event: Value) -> Result<()> {
    if CHAIN.with(|c| c.borrow().len()) >= MAX_DEPTH {
        return Ok(());
    }
    let etype = event["type"].as_str().unwrap_or("").to_string();
    if event.get("eventId").is_none() {
        event["eventId"] = json!(new_id());
    }
    let rules: Vec<(String, Value)> = conn
        .prepare("SELECT id, config FROM automations WHERE enabled=1")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, serde_json::from_str::<Value>(&r.get::<_, String>(1)?).unwrap_or(json!({})))))?
        .collect::<std::result::Result<_, _>>()?;
    for (rid, cfg) in rules {
        let trig = &cfg["trigger"];
        if trig["type"].as_str() != Some(etype.as_str()) {
            continue;
        }
        if etype.starts_with("record_") && trig["databaseId"] != event["databaseId"] {
            continue;
        }
        if etype == "record_updated" {
            if let Some(pid) = trig["propertyId"].as_str().filter(|p| !p.is_empty()) {
                if event["propertyId"].as_str() != Some(pid) {
                    continue;
                }
            }
        }
        if etype == "task_status_changed" {
            if let Some(to) = trig["toStatus"].as_str().filter(|s| !s.is_empty()) {
                if event["to"].as_str() != Some(to) {
                    continue;
                }
            }
        }
        let entity = entity_of(&event);
        if CHAIN.with(|c| c.borrow().iter().any(|(a, e)| a == &rid && e == &entity)) {
            log_run(conn, &rid, "skipped", "Loop prevented: this rule already changed this item in the same chain", None, &event)?;
            continue;
        }
        if !condition_holds(conn, &cfg, &event)? {
            continue;
        }
        let key = event["eventId"].as_str().unwrap_or("").to_string();
        run_rule(conn, &rid, &cfg, &event, Some(&key))?;
    }
    Ok(())
}

fn log_run(conn: &Connection, rid: &str, status: &str, detail: &str, key: Option<&str>, event: &Value) -> Result<bool> {
    let n = conn.execute(
        "INSERT OR IGNORE INTO automation_runs(id, automation_id, status, detail, at, idempotency_key, event_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![new_id(), rid, status, detail, now(), key, event.to_string()],
    )?;
    conn.execute("DELETE FROM automation_runs WHERE automation_id=?1 AND at < ?2", params![rid, (Utc::now() - chrono::Duration::days(60)).to_rfc3339()])?;
    Ok(n > 0)
}

fn condition_holds(conn: &Connection, cfg: &Value, event: &Value) -> Result<bool> {
    let cond = &cfg["condition"];
    if cond.is_null() {
        return Ok(true);
    }
    if let (Some(rid), Some(filter)) = (event["recordId"].as_str(), cond.get("filter")) {
        let db = event["databaseId"].as_str().unwrap_or("");
        let res = databases::query(conn, db, &databases::QuerySpec { filter: Some(filter.clone()), limit: Some(5000), ..Default::default() })?;
        return Ok(res.records.iter().any(|r| r.id == rid));
    }
    if let Some(tid) = event["taskId"].as_str() {
        let t = tasks::get(conn, tid)?;
        if let Some(s) = cond["status"].as_str().filter(|s| !s.is_empty()) {
            if t.status != s {
                return Ok(false);
            }
        }
        if let Some(p) = cond["priority"].as_str().filter(|s| !s.is_empty()) {
            if t.priority != p {
                return Ok(false);
            }
        }
        if cond["dueToday"].as_bool() == Some(true) && t.due_date.as_deref() != Some(tasks::today(conn).as_str()) {
            return Ok(false);
        }
        if let Some(pid) = cond["projectId"].as_str().filter(|s| !s.is_empty()) {
            if t.project_id.as_deref() != Some(pid) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn fill(template: &str, event: &Value, title: &str) -> String {
    template.replace("{{title}}", title).replace("{{date}}", event["date"].as_str().unwrap_or(""))
}

/// Runs all actions of a rule atomically and logs the run. Returns false when the idempotency key was already used.
fn run_rule(conn: &Connection, rid: &str, cfg: &Value, event: &Value, key: Option<&str>) -> Result<bool> {
    if let Some(k) = key {
        let seen: Option<i64> = conn.query_row("SELECT 1 FROM automation_runs WHERE automation_id=?1 AND idempotency_key=?2", params![rid, k], |r| r.get(0)).optional()?;
        if seen.is_some() {
            return Ok(false);
        }
    }
    let entity = entity_of(event);
    CHAIN.with(|c| c.borrow_mut().push((rid.to_string(), entity)));
    let result = (|| -> Result<Vec<String>> {
        let tx = crate::infrastructure::database::begin(conn)?;
        let mut done = vec![];
        for a in cfg["actions"].as_array().into_iter().flatten() {
            done.push(run_action(&tx, a, event)?);
        }
        tx.commit()?;
        Ok(done)
    })();
    CHAIN.with(|c| c.borrow_mut().pop());
    match result {
        Ok(done) => log_run(conn, rid, "ok", &done.join("; "), key, event),
        Err(e) => log_run(conn, rid, "error", &e.to_string(), key, event),
    }
}

fn run_action(conn: &Connection, a: &Value, event: &Value) -> Result<String> {
    let title = if let Some(r) = event["recordId"].as_str() {
        databases::get_record(conn, r).map(|x| x.title).unwrap_or_default()
    } else if let Some(t) = event["taskId"].as_str() {
        tasks::get(conn, t).map(|x| x.title).unwrap_or_default()
    } else {
        String::new()
    };
    match a["type"].as_str().unwrap_or("") {
        "set_property" => {
            let rec = event["recordId"].as_str().ok_or_else(|| AppError::Invalid("no record in this event".into()))?;
            let pid = a["propertyId"].as_str().unwrap_or("");
            databases::set_value(conn, rec, pid, &a["value"])?;
            Ok(format!("set property on “{title}”"))
        }
        "create_task" => {
            let due = a["dueInDays"].as_i64().map(|d| {
                let today = crate::domain::growth::date(&tasks::today(conn)).unwrap();
                (today + chrono::Duration::days(d)).to_string()
            });
            let t = tasks::create(conn, &json!({
                "title": fill(a["title"].as_str().unwrap_or("Follow up: {{title}}"), event, &title),
                "dueDate": due,
                "priority": a["priority"].as_str().unwrap_or("none"),
                "description": format!("Created by an automation{}", if title.is_empty() { String::new() } else { format!(" from “{title}”") }),
                "links": event["recordId"].as_str().map(|r| vec![r]).unwrap_or_default(),
            }))?;
            Ok(format!("created task “{}”", t.title))
        }
        "set_task_status" => {
            let tid = event["taskId"].as_str().ok_or_else(|| AppError::Invalid("no task in this event".into()))?;
            tasks::update(conn, tid, &json!({ "status": a["status"] }))?;
            Ok(format!("set “{title}” to {}", a["status"].as_str().unwrap_or("")))
        }
        "create_page_from_template" => {
            let tpl = a["templateId"].as_str().unwrap_or("");
            let page_title = a["title"].as_str().map(|t| fill(t, event, &title));
            let p = templates::instantiate(conn, tpl, a["parentId"].as_str().filter(|s| !s.is_empty()), None, page_title.as_deref())?;
            Ok(format!("created page “{}”", p.title))
        }
        "notify" => {
            conn.execute(
                "INSERT INTO notification_outbox(id, title, body, target_id, created_at) VALUES (?1,?2,?3,?4,?5)",
                params![new_id(), fill(a["title"].as_str().unwrap_or("{{title}}"), event, &title), fill(a["body"].as_str().unwrap_or(""), event, &title), entity_of(event), now()],
            )?;
            Ok("queued a notification".into())
        }
        other => invalid(format!("unknown action {other}")),
    }
}

/// Manual retry of a failed or skipped run with the same event.
pub fn retry(conn: &Connection, run_id: &str) -> Result<Value> {
    let (rid, event): (String, Option<String>) = conn
        .query_row("SELECT automation_id, event_json FROM automation_runs WHERE id=?1", [run_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?
        .ok_or_else(|| AppError::NotFound("run".into()))?;
    let cfg: String = conn.query_row("SELECT config FROM automations WHERE id=?1", [&rid], |r| r.get(0))?;
    let event: Value = event.and_then(|e| serde_json::from_str(&e).ok()).unwrap_or(json!({}));
    run_rule(conn, &rid, &serde_json::from_str(&cfg)?, &event, Some(&format!("retry:{run_id}:{}", new_id())))?;
    Ok(runs(conn, Some(&rid), 1)?.into_iter().next().unwrap_or(Value::Null))
}

/// Called by the scheduler each tick: runs schedule rules once per day after their time.
pub fn tick_schedules(conn: &Connection) -> Result<()> {
    let tz = settings::user_tz(conn);
    let local = Utc::now().with_timezone(&tz);
    let today = local.format("%Y-%m-%d").to_string();
    let hm = local.format("%H:%M").to_string();
    let wd = local.weekday().number_from_monday() as i64;
    let rules: Vec<(String, Value)> = conn
        .prepare("SELECT id, config FROM automations WHERE enabled=1")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, serde_json::from_str::<Value>(&r.get::<_, String>(1)?).unwrap_or(json!({})))))?
        .collect::<std::result::Result<_, _>>()?;
    for (rid, cfg) in rules {
        let trig = &cfg["trigger"];
        if trig["type"] != "schedule" || trig["time"].as_str().is_none_or(|t| t > hm.as_str()) {
            continue;
        }
        if let Some(days) = trig["weekdays"].as_array().filter(|d| !d.is_empty()) {
            if !days.iter().any(|d| d.as_i64() == Some(wd)) {
                continue;
            }
        }
        match trig["forEach"].as_str() {
            Some(view @ ("tasks_due_today" | "tasks_overdue")) => {
                let list = tasks::list(conn, if view == "tasks_due_today" { "today" } else { "overdue" }, None, None, 200)?;
                for t in list {
                    let event = json!({ "type": "schedule", "date": today, "taskId": t.id });
                    if condition_holds(conn, &cfg, &event)? {
                        run_rule(conn, &rid, &cfg, &event, Some(&format!("schedule:{today}:{}", t.id)))?;
                    }
                }
            }
            _ => {
                let event = json!({ "type": "schedule", "date": today });
                run_rule(conn, &rid, &cfg, &event, Some(&format!("schedule:{today}")))?;
            }
        }
    }
    Ok(())
}

/// Pending notifications for the scheduler to deliver (marked sent).
pub fn take_outbox(conn: &Connection) -> Result<Vec<(String, String, Option<String>)>> {
    let rows: Vec<(String, String, String, Option<String>)> = conn
        .prepare("SELECT id, title, body, target_id FROM notification_outbox WHERE sent_at IS NULL ORDER BY created_at LIMIT 20")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<std::result::Result<_, _>>()?;
    for (id, ..) in &rows {
        conn.execute("UPDATE notification_outbox SET sent_at=?2 WHERE id=?1", params![id, now()])?;
    }
    conn.execute("DELETE FROM notification_outbox WHERE sent_at < ?1", [(Utc::now() - chrono::Duration::days(7)).to_rfc3339()])?;
    Ok(rows.into_iter().map(|(_, t, b, target)| (t, b, target)).collect())
}

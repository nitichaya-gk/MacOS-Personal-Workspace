//! Tasks, Today/Upcoming/Overdue views and reminder scheduling.
//!
//! `due_date` is a calendar date in the user's time zone. `remind_at` is a UTC instant, so a time zone
//! change or a sleeping Mac never shifts when a reminder fires; only "which day is today" follows the zone.

use super::settings::user_tz;
use crate::domain::{new_id, now, TASK_PRIORITIES, TASK_STATUSES};
use crate::error::{invalid, AppError, Result};
use crate::infrastructure::search;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub remind_at: Option<String>,
    /// remind_at shown in the user's time zone as YYYY-MM-DDTHH:MM.
    pub remind_at_local: Option<String>,
    pub project_id: Option<String>,
    pub project_title: Option<String>,
    pub source_page_id: Option<String>,
    pub source_page_title: Option<String>,
    pub completed_at: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub links: Vec<Value>,
    pub milestone_id: Option<String>,
    pub estimated_effort_minutes: Option<i64>,
    pub actual_effort_minutes: Option<i64>,
    /// [{requirementId, requirementTitle, goalId, goalTitle, allocationPct}]
    pub requirement_links: Vec<Value>,
    /// [{id, name}] skills practised (practice is not evidence of growth)
    pub skills: Vec<Value>,
    /// [{id, title, status}]
    pub depends_on: Vec<Value>,
}

const TASK_SELECT: &str = "SELECT t.id, t.title, t.description, t.status, t.priority, t.due_date, t.remind_at, t.project_id, pr.title,
    t.source_page_id, sp.title, t.completed_at, t.deleted_at, t.created_at, t.updated_at,
    t.milestone_id, t.estimated_effort_minutes, t.actual_effort_minutes
    FROM tasks t LEFT JOIN pages pr ON pr.id = t.project_id LEFT JOIN pages sp ON sp.id = t.source_page_id";

fn row_to_task(r: &Row, tz: &chrono_tz::Tz) -> rusqlite::Result<Task> {
    let remind_at: Option<String> = r.get(6)?;
    Ok(Task {
        id: r.get(0)?,
        title: r.get(1)?,
        description: r.get(2)?,
        status: r.get(3)?,
        priority: r.get(4)?,
        due_date: r.get(5)?,
        remind_at_local: remind_at.as_deref().and_then(|s| to_local(s, tz)),
        remind_at,
        project_id: r.get(7)?,
        project_title: r.get(8)?,
        source_page_id: r.get(9)?,
        source_page_title: r.get(10)?,
        completed_at: r.get(11)?,
        deleted_at: r.get(12)?,
        created_at: r.get(13)?,
        updated_at: r.get(14)?,
        links: vec![],
        milestone_id: r.get(15)?,
        estimated_effort_minutes: r.get(16)?,
        actual_effort_minutes: r.get(17)?,
        requirement_links: vec![],
        skills: vec![],
        depends_on: vec![],
    })
}

fn to_local(utc: &str, tz: &chrono_tz::Tz) -> Option<String> {
    DateTime::parse_from_rfc3339(utc).ok().map(|d| d.with_timezone(tz).format("%Y-%m-%dT%H:%M").to_string())
}

/// Local "YYYY-MM-DDTHH:MM" in the user's zone → UTC RFC3339. DST gaps resolve to the later valid time.
pub fn local_to_utc(local: &str, tz: &chrono_tz::Tz) -> Result<String> {
    let naive = NaiveDateTime::parse_from_str(local, "%Y-%m-%dT%H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(local, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|_| AppError::Invalid("reminder time must be YYYY-MM-DDTHH:MM".into()))?;
    let dt = tz
        .from_local_datetime(&naive)
        .earliest()
        .or_else(|| tz.from_local_datetime(&(naive + chrono::Duration::hours(1))).earliest())
        .ok_or_else(|| AppError::Invalid("time does not exist in this time zone".into()))?;
    Ok(dt.with_timezone(&Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

pub fn today(conn: &Connection) -> String {
    Utc::now().with_timezone(&user_tz(conn)).format("%Y-%m-%d").to_string()
}

fn with_links(conn: &Connection, mut t: Task) -> Result<Task> {
    let mut stmt = conn.prepare(
        "SELECT l.target_id, p.title, p.kind, p.icon FROM links l JOIN pages p ON p.id = l.target_id WHERE l.source_id=?1 AND p.deleted_at IS NULL",
    )?;
    t.links = stmt
        .query_map([&t.id], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "kind": r.get::<_, String>(2)?, "icon": r.get::<_, Option<String>>(3)? })))?
        .collect::<std::result::Result<_, _>>()?;
    t.requirement_links = conn
        .prepare(
            "SELECT l.requirement_id, r.title, g.id, g.title, l.allocation_pct FROM task_requirement_links l
             JOIN goal_requirements r ON r.id = l.requirement_id JOIN goals g ON g.id = r.goal_id WHERE l.task_id=?1 ORDER BY g.title, r.position",
        )?
        .query_map([&t.id], |r| {
            Ok(json!({ "requirementId": r.get::<_, String>(0)?, "requirementTitle": r.get::<_, String>(1)?, "goalId": r.get::<_, String>(2)?, "goalTitle": r.get::<_, String>(3)?, "allocationPct": r.get::<_, f64>(4)? }))
        })?
        .collect::<std::result::Result<_, _>>()?;
    t.skills = conn
        .prepare("SELECT s.id, s.name FROM task_skill_links l JOIN skills s ON s.id = l.skill_id WHERE l.task_id=?1 AND s.deleted_at IS NULL ORDER BY s.name")?
        .query_map([&t.id], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)? })))?
        .collect::<std::result::Result<_, _>>()?;
    t.depends_on = conn
        .prepare("SELECT d.depends_on_id, x.title, x.status FROM task_dependencies d JOIN tasks x ON x.id = d.depends_on_id WHERE d.task_id=?1 AND x.deleted_at IS NULL")?
        .query_map([&t.id], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "status": r.get::<_, String>(2)? })))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(t)
}

/// Would `task` depending on `on` create a cycle (on already depends, transitively, on task)?
fn creates_cycle(conn: &Connection, task: &str, on: &str) -> Result<bool> {
    if task == on {
        return Ok(true);
    }
    let found: Option<i64> = conn
        .query_row(
            "WITH RECURSIVE up(id) AS (SELECT ?1 UNION SELECT d.depends_on_id FROM task_dependencies d JOIN up ON d.task_id = up.id)
             SELECT 1 FROM up WHERE id = ?2 LIMIT 1",
            params![on, task],
            |r| r.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

fn set_growth_links(conn: &Connection, id: &str, patch: &Value) -> Result<()> {
    if let Some(links) = patch.get("requirementLinks").and_then(|l| l.as_array()) {
        let mut total = 0.0;
        let mut rows = vec![];
        for l in links {
            let rid = l.get("requirementId").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("requirementId missing".into()))?;
            let pct = l.get("allocationPct").and_then(|v| v.as_f64()).unwrap_or(100.0);
            if !(pct > 0.0 && pct <= 100.0) {
                return invalid("allocation must be between 1 and 100%");
            }
            total += pct;
            rows.push((rid.to_string(), pct, l.get("rationale").and_then(|v| v.as_str()).unwrap_or("").to_string()));
        }
        // A task's effort is never counted more than once across requirements.
        if total > 100.0 + 1e-9 {
            return invalid(format!("allocations add up to {total:.0}% — the total for one task can be at most 100%"));
        }
        conn.execute("DELETE FROM task_requirement_links WHERE task_id=?1", [id])?;
        for (rid, pct, why) in rows {
            conn.execute(
                "INSERT INTO task_requirement_links(task_id, requirement_id, allocation_pct, rationale, created_at) VALUES (?1,?2,?3,?4,?5)",
                params![id, rid, pct, why, now()],
            )?;
        }
    }
    if let Some(skills) = patch.get("skillIds").and_then(|l| l.as_array()) {
        conn.execute("DELETE FROM task_skill_links WHERE task_id=?1", [id])?;
        for s in skills.iter().filter_map(|s| s.as_str()) {
            conn.execute("INSERT OR IGNORE INTO task_skill_links(task_id, skill_id) VALUES (?1,?2)", params![id, s])?;
        }
    }
    if let Some(deps) = patch.get("dependsOn").and_then(|l| l.as_array()) {
        conn.execute("DELETE FROM task_dependencies WHERE task_id=?1", [id])?;
        for on in deps.iter().filter_map(|s| s.as_str()) {
            if creates_cycle(conn, id, on)? {
                return invalid("that dependency would create a cycle");
            }
            conn.execute("INSERT OR IGNORE INTO task_dependencies(task_id, depends_on_id, created_at) VALUES (?1,?2,?3)", params![id, on, now()])?;
        }
    }
    for (col, key) in [("estimated_effort_minutes", "estimatedEffortMinutes"), ("actual_effort_minutes", "actualEffortMinutes")] {
        if let Some(v) = patch.get(key) {
            let n = v.as_i64();
            if n.is_some_and(|n| n < 0) {
                return invalid("effort cannot be negative");
            }
            conn.execute(&format!("UPDATE tasks SET {col}=?2 WHERE id=?1"), params![id, n])?;
        }
    }
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> Result<Task> {
    let tz = user_tz(conn);
    let t = conn
        .query_row(&format!("{TASK_SELECT} WHERE t.id=?1"), [id], |r| row_to_task(r, &tz))
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("task {id}")))?;
    with_links(conn, t)
}

/// view: today | upcoming | overdue | completed | nodate | all | open
pub fn list(conn: &Connection, view: &str, project_id: Option<&str>, page_id: Option<&str>, limit: usize) -> Result<Vec<Task>> {
    let tz = user_tz(conn);
    let today = today(conn);
    let cond = match view {
        "today" => "t.status<>'done' AND t.due_date = ?1",
        "overdue" => "t.status<>'done' AND t.due_date < ?1",
        "upcoming" => "t.status<>'done' AND t.due_date > ?1",
        "nodate" => "t.status<>'done' AND t.due_date IS NULL AND ?1 IS NOT NULL",
        "completed" => "t.status='done' AND ?1 IS NOT NULL",
        "open" => "t.status<>'done' AND ?1 IS NOT NULL",
        "all" => "?1 IS NOT NULL",
        _ => return invalid("unknown task view"),
    };
    let order = if view == "completed" {
        "t.completed_at DESC"
    } else {
        "t.due_date IS NULL, t.due_date, CASE t.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END, t.created_at"
    };
    let sql = format!(
        "{TASK_SELECT} WHERE t.deleted_at IS NULL AND {cond} AND (?2 IS NULL OR t.project_id = ?2) AND (?3 IS NULL OR t.source_page_id = ?3) ORDER BY {order} LIMIT ?4"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![today, project_id, page_id, limit as i64], |r| row_to_task(r, &tz))?;
    let tasks: Vec<Task> = rows.collect::<std::result::Result<_, _>>()?;
    tasks.into_iter().map(|t| with_links(conn, t)).collect()
}

pub fn counts(conn: &Connection) -> Result<Value> {
    let today = today(conn);
    let c = |sql: &str| -> Result<i64> { Ok(conn.query_row(sql, [&today], |r| r.get(0))?) };
    Ok(json!({
        "today": c("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL AND status<>'done' AND due_date = ?1")?,
        "overdue": c("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL AND status<>'done' AND due_date < ?1")?,
        "upcoming": c("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL AND status<>'done' AND due_date > ?1")?,
        "date": today,
        "timezone": user_tz(conn).name(),
    }))
}

fn validate_patch(patch: &Value) -> Result<()> {
    if let Some(s) = patch.get("status").and_then(|s| s.as_str()) {
        if !TASK_STATUSES.contains(&s) {
            return invalid("status must be todo, in_progress or done");
        }
    }
    if let Some(p) = patch.get("priority").and_then(|s| s.as_str()) {
        if !TASK_PRIORITIES.contains(&p) {
            return invalid("unknown priority");
        }
    }
    if let Some(d) = patch.get("dueDate").and_then(|s| s.as_str()) {
        if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
            return invalid("due date must be YYYY-MM-DD");
        }
    }
    if let Some(t) = patch.get("title").and_then(|s| s.as_str()) {
        if t.trim().is_empty() {
            return invalid("task title is required");
        }
    }
    Ok(())
}

pub fn create(conn: &Connection, p: &Value) -> Result<Task> {
    validate_patch(p)?;
    let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
    if title.is_empty() {
        return invalid("task title is required");
    }
    let id = new_id();
    let ts = now();
    conn.execute(
        "INSERT INTO tasks(id, title, description, status, priority, project_id, source_page_id, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)",
        params![
            id,
            title,
            p.get("description").and_then(|t| t.as_str()).unwrap_or(""),
            p.get("status").and_then(|t| t.as_str()).unwrap_or("todo"),
            p.get("priority").and_then(|t| t.as_str()).unwrap_or("none"),
            p.get("projectId").and_then(|t| t.as_str()),
            p.get("sourcePageId").and_then(|t| t.as_str()),
            ts
        ],
    )?;
    let mut rest = p.clone();
    if let Some(o) = rest.as_object_mut() {
        for k in ["title", "description", "status", "priority", "projectId", "sourcePageId"] {
            // remaining keys (dates, reminder, links, milestone, effort…) are applied by update()
            o.remove(k);
        }
    }
    update(conn, &id, &rest)
}

pub fn update(conn: &Connection, id: &str, patch: &Value) -> Result<Task> {
    validate_patch(patch)?;
    let before = get(conn, id)?;
    let tz = user_tz(conn);
    let ts = now();
    let tx = crate::infrastructure::database::begin(conn)?;
    let set_text = |col: &str, key: &str| -> Result<()> {
        if let Some(v) = patch.get(key) {
            tx.execute(&format!("UPDATE tasks SET {col}=?2 WHERE id=?1"), params![id, v.as_str().map(str::trim)])?;
        }
        Ok(())
    };
    set_text("title", "title")?;
    set_text("description", "description")?;
    set_text("priority", "priority")?;
    set_text("due_date", "dueDate")?;
    set_text("project_id", "projectId")?;
    set_text("source_page_id", "sourcePageId")?;
    set_text("milestone_id", "milestoneId")?;
    set_growth_links(&tx, id, patch)?;
    if let Some(s) = patch.get("status").and_then(|s| s.as_str()) {
        let completed = if s == "done" { before.completed_at.clone().or(Some(ts.clone())) } else { None };
        tx.execute("UPDATE tasks SET status=?2, completed_at=?3 WHERE id=?1", params![id, s, completed])?;
    }
    if let Some(r) = patch.get("remindAtLocal") {
        let utc = match r.as_str().filter(|s| !s.is_empty()) {
            Some(local) => Some(local_to_utc(local, &tz)?),
            None => None,
        };
        tx.execute("UPDATE tasks SET remind_at=?2 WHERE id=?1", params![id, utc])?;
    }
    if let Some(links) = patch.get("links").and_then(|l| l.as_array()) {
        let targets: Vec<(String, String)> = links.iter().filter_map(|l| l.as_str()).map(|s| (s.to_string(), String::new())).collect();
        super::pages::set_links(&tx, id, "task", &targets)?;
    }
    tx.execute("UPDATE tasks SET updated_at=?2 WHERE id=?1", params![id, ts])?;
    reschedule(&tx, id)?;
    tx.commit()?;
    index(conn, id)?;
    get(conn, id)
}

/// Keeps exactly one pending reminder per open task with a future reminder time.
/// Called on every change, so completing, rescheduling or deleting a task never leaves a stale reminder.
pub fn reschedule(conn: &Connection, id: &str) -> Result<()> {
    let (status, remind_at, deleted): (String, Option<String>, Option<String>) =
        conn.query_row("SELECT status, remind_at, deleted_at FROM tasks WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let pending: Option<(String, String)> = conn
        .query_row("SELECT id, fire_at FROM reminders WHERE task_id=?1 AND status='pending'", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let wanted = match (&remind_at, status.as_str(), &deleted) {
        (Some(at), s, None) if s != "done" && at.as_str() > now().as_str() => Some(at.clone()),
        _ => None,
    };
    match (pending, wanted) {
        (Some((_, fire_at)), Some(w)) if fire_at == w => {}
        (pending, wanted) => {
            if let Some((rid, _)) = pending {
                conn.execute("UPDATE reminders SET status='cancelled' WHERE id=?1", [rid])?;
            }
            if let Some(w) = wanted {
                conn.execute("INSERT INTO reminders(id, task_id, fire_at, status, created_at) VALUES (?1,?2,?3,'pending',?4)", params![new_id(), id, w, now()])?;
            }
        }
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    get(conn, id)?;
    conn.execute("UPDATE tasks SET deleted_at=?2, updated_at=?2 WHERE id=?1", params![id, now()])?;
    reschedule(conn, id)
}

pub fn restore(conn: &Connection, id: &str) -> Result<Task> {
    conn.execute("UPDATE tasks SET deleted_at=NULL WHERE id=?1", [id])?;
    reschedule(conn, id)?;
    get(conn, id)
}

pub fn delete_forever(conn: &Connection, id: &str) -> Result<()> {
    search::remove(conn, id)?;
    conn.execute("DELETE FROM links WHERE source_id=?1", [id])?;
    conn.execute("DELETE FROM tasks WHERE id=?1 AND deleted_at IS NOT NULL", [id])?;
    Ok(())
}

pub fn list_trash(conn: &Connection) -> Result<Vec<Task>> {
    let tz = user_tz(conn);
    let mut stmt = conn.prepare(&format!("{TASK_SELECT} WHERE t.deleted_at IS NOT NULL ORDER BY t.deleted_at DESC"))?;
    let rows = stmt.query_map([], |r| row_to_task(r, &tz))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn index(conn: &Connection, id: &str) -> Result<()> {
    let t = get(conn, id)?;
    search::upsert(conn, id, "task", t.project_id.as_deref(), &t.updated_at, &t.title, &t.description)
}

// ---------- reminder delivery ----------

pub struct DueReminder {
    pub task: Task,
}

pub struct ReminderBatch {
    pub fire: Vec<DueReminder>,
    pub missed: Vec<DueReminder>,
}

/// Claims reminders whose time has come. Those overdue by more than `grace_secs` (the app was closed
/// or the Mac slept) are marked missed so they can be summarised once instead of firing one by one.
pub fn take_due_reminders(conn: &Connection, grace_secs: i64) -> Result<ReminderBatch> {
    let now_ts = now();
    let cutoff = (Utc::now() - chrono::Duration::seconds(grace_secs)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let due: Vec<(String, String, String)> = conn
        .prepare("SELECT id, task_id, fire_at FROM reminders WHERE status='pending' AND fire_at <= ?1 ORDER BY fire_at")?
        .query_map([&now_ts], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut batch = ReminderBatch { fire: vec![], missed: vec![] };
    for (rid, task_id, fire_at) in due {
        let missed = fire_at < cutoff;
        conn.execute(
            "UPDATE reminders SET status=?2, fired_at=?3 WHERE id=?1 AND status='pending'",
            params![rid, if missed { "missed" } else { "fired" }, now_ts],
        )?;
        let Ok(task) = get(conn, &task_id) else { continue };
        if task.status == "done" || task.deleted_at.is_some() {
            continue;
        }
        if missed { batch.missed.push(DueReminder { task }) } else { batch.fire.push(DueReminder { task }) }
    }
    Ok(batch)
}

pub fn list_missed(conn: &Connection) -> Result<Vec<Task>> {
    let ids: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT r.task_id FROM reminders r JOIN tasks t ON t.id=r.task_id
             WHERE r.status='missed' AND t.status<>'done' AND t.deleted_at IS NULL AND r.fired_at >= ?1",
        )?
        .query_map([(Utc::now() - chrono::Duration::days(2)).to_rfc3339()], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    ids.iter().map(|id| get(conn, id)).collect()
}

pub fn dismiss_missed(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE reminders SET status='cancelled' WHERE status='missed'", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_local_reminder_time() {
        let tz: chrono_tz::Tz = "Asia/Bangkok".parse().unwrap();
        assert_eq!(local_to_utc("2026-10-21T09:00", &tz).unwrap(), "2026-10-21T02:00:00.000Z");
        assert!(local_to_utc("tomorrow", &tz).is_err());
    }
}

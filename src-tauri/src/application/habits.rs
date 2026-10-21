//! Habits: schedules, one log per (habit, date) with done / missed / skipped, streaks and consistency.
//! Habit completion is a process indicator, never an outcome.

use super::tasks;
use crate::domain::growth::{self, habit_stats};
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashMap;

fn validate_schedule(s: &Value) -> Result<()> {
    match s.get("type").and_then(|t| t.as_str()) {
        Some("daily") => Ok(()),
        Some("weekdays") => {
            let days = s.get("days").and_then(|d| d.as_array()).ok_or_else(|| AppError::Invalid("choose at least one weekday".into()))?;
            if days.is_empty() || !days.iter().all(|d| d.as_i64().is_some_and(|d| (1..=7).contains(&d))) {
                return invalid("weekdays are 1 (Monday) to 7 (Sunday)");
            }
            Ok(())
        }
        Some("weekly") => {
            let n = s.get("perWeek").and_then(|d| d.as_i64()).unwrap_or(0);
            if !(1..=7).contains(&n) { invalid("times per week must be 1–7") } else { Ok(()) }
        }
        _ => invalid("schedule type must be daily, weekdays or weekly"),
    }
}

fn today(conn: &Connection) -> NaiveDate {
    growth::date(&tasks::today(conn)).unwrap()
}

fn logs(conn: &Connection, habit_id: &str) -> Result<HashMap<NaiveDate, String>> {
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT scheduled_date, status FROM habit_logs WHERE habit_id=?1")?
        .query_map([habit_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows.into_iter().filter_map(|(d, s)| growth::date(&d).map(|d| (d, s))).collect())
}

pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    let mut h = conn
        .query_row(
            "SELECT h.id, h.title, h.description, h.kind, h.target_value, h.unit, h.schedule_json, h.goal_id, h.skill_id, h.reminder_time, h.start_date,
                    h.archived_at, g.title, s.name
             FROM habits h LEFT JOIN goals g ON g.id=h.goal_id LEFT JOIN skills s ON s.id=h.skill_id WHERE h.id=?1",
            [id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "description": r.get::<_, String>(2)?, "kind": r.get::<_, String>(3)?,
                    "targetValue": r.get::<_, Option<f64>>(4)?, "unit": r.get::<_, String>(5)?,
                    "schedule": serde_json::from_str::<Value>(&r.get::<_, String>(6)?).unwrap_or(json!({"type":"daily"})),
                    "goalId": r.get::<_, Option<String>>(7)?, "skillId": r.get::<_, Option<String>>(8)?, "reminderTime": r.get::<_, Option<String>>(9)?,
                    "startDate": r.get::<_, String>(10)?, "archivedAt": r.get::<_, Option<String>>(11)?,
                    "goalTitle": r.get::<_, Option<String>>(12)?, "skillName": r.get::<_, Option<String>>(13)?,
                }))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("habit".into()))?;
    let t = today(conn);
    let lg = logs(conn, id)?;
    let start = growth::date(h["startDate"].as_str().unwrap_or("")).unwrap_or(t);
    h["stats"] = serde_json::to_value(habit_stats(&h["schedule"], start, &lg, t))?;
    // Last 14 days for the grid: status per date, or not_due / open.
    h["recent"] = json!((0..14)
        .rev()
        .map(|i| {
            let d = t - Duration::days(i);
            let status = lg.get(&d).cloned().unwrap_or_else(|| {
                if d < start || !growth::is_due(&h["schedule"], d) { "not_due".into() } else if d == t { "open".into() } else { "missed".into() }
            });
            json!({ "date": d.to_string(), "status": status, "logged": lg.contains_key(&d) })
        })
        .collect::<Vec<_>>());
    Ok(h)
}

pub fn list(conn: &Connection, include_archived: bool) -> Result<Vec<Value>> {
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM habits WHERE (?1=1 OR archived_at IS NULL) ORDER BY created_at")?
        .query_map([include_archived as i64], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    ids.iter().map(|id| get(conn, id)).collect()
}

pub fn create(conn: &Connection, p: &Value) -> Result<Value> {
    let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
    if title.is_empty() {
        return invalid("habit name is required");
    }
    let id = new_id();
    conn.execute(
        "INSERT INTO habits(id, title, start_date, created_at, updated_at) VALUES (?1,?2,?3,?4,?4)",
        params![id, title, tasks::today(conn), now()],
    )?;
    let mut rest = p.clone();
    rest.as_object_mut().map(|o| o.remove("title"));
    update(conn, &id, &rest)
}

pub fn update(conn: &Connection, id: &str, p: &Value) -> Result<Value> {
    get(conn, id)?;
    for (col, key) in [("title", "title"), ("description", "description"), ("unit", "unit")] {
        if let Some(v) = p.get(key).and_then(|v| v.as_str()) {
            if col == "title" && v.trim().is_empty() {
                return invalid("habit name is required");
            }
            conn.execute(&format!("UPDATE habits SET {col}=?2 WHERE id=?1"), params![id, v.trim()])?;
        }
    }
    if let Some(k) = p.get("kind").and_then(|v| v.as_str()) {
        if !matches!(k, "binary" | "count" | "duration" | "numeric") {
            return invalid("unknown habit type");
        }
        conn.execute("UPDATE habits SET kind=?2 WHERE id=?1", params![id, k])?;
    }
    if let Some(v) = p.get("targetValue") {
        conn.execute("UPDATE habits SET target_value=?2 WHERE id=?1", params![id, v.as_f64()])?;
    }
    if let Some(s) = p.get("schedule") {
        validate_schedule(s)?;
        conn.execute("UPDATE habits SET schedule_json=?2 WHERE id=?1", params![id, s.to_string()])?;
    }
    for (col, key) in [("goal_id", "goalId"), ("skill_id", "skillId"), ("reminder_time", "reminderTime")] {
        if let Some(v) = p.get(key) {
            let v = v.as_str().filter(|s| !s.is_empty());
            if col == "reminder_time" && v.is_some_and(|t| chrono::NaiveTime::parse_from_str(t, "%H:%M").is_err()) {
                return invalid("reminder time must be HH:MM");
            }
            conn.execute(&format!("UPDATE habits SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    if let Some(a) = p.get("archived").and_then(|v| v.as_bool()) {
        conn.execute("UPDATE habits SET archived_at=?2 WHERE id=?1", params![id, a.then(now)])?;
    }
    conn.execute("UPDATE habits SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM habits WHERE id=?1", [id])?;
    Ok(())
}

/// Sets (or clears, with status null) the log for one date. Unique per (habit, date), so check-ins never double count.
pub fn log(conn: &Connection, habit_id: &str, date: &str, status: Option<&str>, value: Option<f64>, note: &str) -> Result<Value> {
    get(conn, habit_id)?;
    let d = growth::date(date).ok_or_else(|| AppError::Invalid("date must be YYYY-MM-DD".into()))?;
    if d > today(conn) {
        return invalid("cannot log a future day");
    }
    match status {
        None => {
            conn.execute("DELETE FROM habit_logs WHERE habit_id=?1 AND scheduled_date=?2", params![habit_id, date])?;
        }
        Some(s) => {
            if !matches!(s, "done" | "missed" | "skipped") {
                return invalid("status must be done, missed or skipped");
            }
            conn.execute(
                "INSERT INTO habit_logs(id, habit_id, scheduled_date, status, value, note, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(habit_id, scheduled_date) DO UPDATE SET status=excluded.status, value=excluded.value, note=excluded.note",
                params![new_id(), habit_id, date, s, value, note, now()],
            )?;
        }
    }
    get(conn, habit_id)
}

pub fn due_today(conn: &Connection) -> Result<Vec<Value>> {
    Ok(list(conn, false)?.into_iter().filter(|h| h["stats"]["dueToday"] == true).collect())
}

/// Logs in [from, to] for reviews.
pub fn consistency_between(conn: &Connection, from: NaiveDate, to: NaiveDate) -> Result<Vec<Value>> {
    let mut out = vec![];
    for h in list(conn, false)? {
        let id = h["id"].as_str().unwrap_or("");
        let lg = logs(conn, id)?;
        let start = growth::date(h["startDate"].as_str().unwrap_or("")).unwrap_or(from);
        let (mut due, mut done, mut skipped) = (0, 0, 0);
        let mut d = from;
        while d <= to {
            if d >= start && growth::is_due(&h["schedule"], d) {
                due += 1;
                match lg.get(&d).map(String::as_str) {
                    Some("done") => done += 1,
                    Some("skipped") => skipped += 1,
                    _ => {}
                }
            }
            d += Duration::days(1);
        }
        let target = growth::per_week(&h["schedule"]);
        let rate = match target {
            Some(t) => Some((done as f64 / t as f64).min(1.0)),
            None if due - skipped > 0 => Some(done as f64 / (due - skipped) as f64),
            None => None,
        };
        out.push(json!({ "id": id, "title": h["title"], "due": due, "done": done, "skipped": skipped, "perWeek": target, "consistency": rate }));
    }
    Ok(out)
}

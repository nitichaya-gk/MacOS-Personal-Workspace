//! Timetable: planned time blocks by day and time (one-off or weekly), shown together with tasks due
//! that day and the focus time actually logged — plan vs. actual.

use super::{goals, settings, skills, tasks};
use crate::domain::growth;
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use chrono::{Datelike, Duration, NaiveDate, NaiveTime, TimeZone};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{json, Value};

pub const KINDS: &[&str] = &["study", "work", "class", "habit", "personal", "other"];

const COLS: &str = "b.id, b.title, b.kind, b.recurrence, b.date, b.weekdays, b.start_time, b.end_time, b.valid_from, b.valid_until, b.skip_dates,
    b.task_id, b.goal_id, b.habit_id, b.location, b.notes, b.remind_minutes, t.title, g.title, h.title";

fn row(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "kind": r.get::<_, String>(2)?, "recurrence": r.get::<_, String>(3)?,
        "date": r.get::<_, Option<String>>(4)?, "weekdays": serde_json::from_str::<Value>(&r.get::<_, String>(5)?).unwrap_or(json!([])),
        "startTime": r.get::<_, String>(6)?, "endTime": r.get::<_, String>(7)?, "validFrom": r.get::<_, Option<String>>(8)?,
        "validUntil": r.get::<_, Option<String>>(9)?, "skipDates": serde_json::from_str::<Value>(&r.get::<_, String>(10)?).unwrap_or(json!([])),
        "taskId": r.get::<_, Option<String>>(11)?, "goalId": r.get::<_, Option<String>>(12)?, "habitId": r.get::<_, Option<String>>(13)?,
        "location": r.get::<_, String>(14)?, "notes": r.get::<_, String>(15)?, "remindMinutes": r.get::<_, Option<i64>>(16)?,
        "taskTitle": r.get::<_, Option<String>>(17)?, "goalTitle": r.get::<_, Option<String>>(18)?, "habitTitle": r.get::<_, Option<String>>(19)?,
    }))
}

fn select() -> String {
    format!("SELECT {COLS} FROM time_blocks b LEFT JOIN tasks t ON t.id=b.task_id LEFT JOIN goals g ON g.id=b.goal_id LEFT JOIN habits h ON h.id=b.habit_id")
}

pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    conn.query_row(&format!("{} WHERE b.id=?1", select()), [id], row).optional()?.ok_or_else(|| AppError::NotFound("time block".into()))
}

fn minutes(t: &str) -> Option<i64> {
    NaiveTime::parse_from_str(t, "%H:%M").ok().map(|x| x.hour_min())
}

trait HourMin {
    fn hour_min(&self) -> i64;
}
impl HourMin for NaiveTime {
    fn hour_min(&self) -> i64 {
        use chrono::Timelike;
        self.hour() as i64 * 60 + self.minute() as i64
    }
}

/// Does a block occur on this date?
fn occurs(b: &Value, d: NaiveDate) -> bool {
    let ds = d.to_string();
    if b["skipDates"].as_array().is_some_and(|s| s.iter().any(|x| x.as_str() == Some(ds.as_str()))) {
        return false;
    }
    match b["recurrence"].as_str() {
        Some("weekly") => {
            let wd = d.weekday().number_from_monday() as i64;
            b["weekdays"].as_array().is_some_and(|w| w.iter().any(|x| x.as_i64() == Some(wd)))
                && b["validFrom"].as_str().is_none_or(|f| ds.as_str() >= f)
                && b["validUntil"].as_str().is_none_or(|u| ds.as_str() <= u)
        }
        _ => b["date"].as_str() == Some(ds.as_str()),
    }
}

fn validate_and_write(conn: &Connection, id: &str, p: &Value) -> Result<()> {
    let s = |k: &str| p.get(k).map(|v| v.as_str().map(str::trim).filter(|x| !x.is_empty()).map(String::from));
    if let Some(t) = s("title") {
        conn.execute("UPDATE time_blocks SET title=?2 WHERE id=?1", params![id, t.ok_or_else(|| AppError::Invalid("give the block a name".into()))?])?;
    }
    if let Some(k) = p.get("kind").and_then(|v| v.as_str()) {
        if !KINDS.contains(&k) {
            return invalid("unknown block type");
        }
        conn.execute("UPDATE time_blocks SET kind=?2 WHERE id=?1", params![id, k])?;
    }
    if let Some(r) = p.get("recurrence").and_then(|v| v.as_str()) {
        if !matches!(r, "once" | "weekly") {
            return invalid("repeat must be once or weekly");
        }
        conn.execute("UPDATE time_blocks SET recurrence=?2 WHERE id=?1", params![id, r])?;
    }
    for (col, key) in [("date", "date"), ("valid_from", "validFrom"), ("valid_until", "validUntil")] {
        if let Some(v) = s(key) {
            if v.as_deref().is_some_and(|d| growth::date(d).is_none()) {
                return invalid("dates must be YYYY-MM-DD");
            }
            conn.execute(&format!("UPDATE time_blocks SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    for (col, key) in [("start_time", "startTime"), ("end_time", "endTime")] {
        if let Some(Some(v)) = s(key) {
            if minutes(&v).is_none() {
                return invalid("times must be HH:MM");
            }
            conn.execute(&format!("UPDATE time_blocks SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    if let Some(w) = p.get("weekdays").and_then(|v| v.as_array()) {
        if !w.iter().all(|d| d.as_i64().is_some_and(|d| (1..=7).contains(&d))) {
            return invalid("weekdays are 1 (Monday) to 7 (Sunday)");
        }
        conn.execute("UPDATE time_blocks SET weekdays=?2 WHERE id=?1", params![id, Value::Array(w.clone()).to_string()])?;
    }
    for (col, key) in [("task_id", "taskId"), ("goal_id", "goalId"), ("habit_id", "habitId")] {
        if let Some(v) = s(key) {
            conn.execute(&format!("UPDATE time_blocks SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    for (col, key) in [("location", "location"), ("notes", "notes")] {
        if let Some(v) = p.get(key).and_then(|v| v.as_str()) {
            conn.execute(&format!("UPDATE time_blocks SET {col}=?2 WHERE id=?1"), params![id, v.trim()])?;
        }
    }
    if let Some(v) = p.get("remindMinutes") {
        let m = v.as_i64();
        if m.is_some_and(|m| !(0..=1440).contains(&m)) {
            return invalid("reminder must be 0–1440 minutes before");
        }
        conn.execute("UPDATE time_blocks SET remind_minutes=?2 WHERE id=?1", params![id, m])?;
    }
    // Whole-block checks after the update.
    let b = get(conn, id)?;
    let (st, en) = (minutes(b["startTime"].as_str().unwrap_or("")), minutes(b["endTime"].as_str().unwrap_or("")));
    if !matches!((st, en), (Some(a), Some(z)) if z > a) {
        return invalid("the end time must be after the start time");
    }
    match b["recurrence"].as_str() {
        Some("weekly") if b["weekdays"].as_array().is_none_or(|w| w.is_empty()) => return invalid("choose at least one weekday"),
        Some("once") if b["date"].is_null() => return invalid("choose a date"),
        _ => {}
    }
    if let (Some(f), Some(u)) = (b["validFrom"].as_str(), b["validUntil"].as_str()) {
        if u < f {
            return invalid("the series ends before it starts");
        }
    }
    conn.execute("UPDATE time_blocks SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    Ok(())
}

pub fn create(conn: &Connection, p: &Value) -> Result<Value> {
    let id = new_id();
    let ts = now();
    let tx = crate::infrastructure::database::begin(conn)?;
    tx.execute(
        "INSERT INTO time_blocks(id, title, start_time, end_time, created_at, updated_at) VALUES (?1,'Block','09:00','10:00',?2,?2)",
        params![id, ts],
    )?;
    validate_and_write(&tx, &id, p)?;
    tx.commit()?;
    get(conn, &id)
}

pub fn update(conn: &Connection, id: &str, p: &Value) -> Result<Value> {
    get(conn, id)?;
    let tx = crate::infrastructure::database::begin(conn)?;
    validate_and_write(&tx, id, p)?;
    tx.commit()?;
    get(conn, id)
}

/// Changes one occurrence of a weekly block: the date is skipped in the series and a one-off copy holds the change.
pub fn update_occurrence(conn: &Connection, id: &str, date: &str, p: &Value) -> Result<Value> {
    let b = get(conn, id)?;
    if b["recurrence"] != "weekly" {
        return update(conn, id, p);
    }
    let tx = crate::infrastructure::database::begin(conn)?;
    skip_date(&tx, id, date)?;
    let mut copy = json!({
        "title": b["title"], "kind": b["kind"], "recurrence": "once", "date": date, "startTime": b["startTime"], "endTime": b["endTime"],
        "taskId": b["taskId"], "goalId": b["goalId"], "habitId": b["habitId"], "location": b["location"], "notes": b["notes"], "remindMinutes": b["remindMinutes"],
    });
    for (k, v) in p.as_object().into_iter().flatten() {
        copy[k] = v.clone();
    }
    copy["recurrence"] = json!("once");
    let out = create(&tx, &copy)?;
    tx.commit()?;
    Ok(out)
}

fn skip_date(conn: &Connection, id: &str, date: &str) -> Result<()> {
    let b = get(conn, id)?;
    let mut skips: Vec<Value> = b["skipDates"].as_array().cloned().unwrap_or_default();
    if !skips.iter().any(|x| x.as_str() == Some(date)) {
        skips.push(json!(date));
    }
    conn.execute("UPDATE time_blocks SET skip_dates=?2, updated_at=?3 WHERE id=?1", params![id, Value::Array(skips).to_string(), now()])?;
    Ok(())
}

/// Deletes a block, or with `date` only that occurrence of a weekly block.
pub fn delete(conn: &Connection, id: &str, date: Option<&str>) -> Result<()> {
    let b = get(conn, id)?;
    match date {
        Some(d) if b["recurrence"] == "weekly" => skip_date(conn, id, d),
        _ => {
            conn.execute("UPDATE time_blocks SET deleted_at=?2 WHERE id=?1", params![id, now()])?;
            Ok(())
        }
    }
}

fn all_blocks(conn: &Connection) -> Result<Vec<Value>> {
    let rows = conn.prepare(&format!("{} WHERE b.deleted_at IS NULL", select()))?.query_map([], row)?.collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

pub fn occurrences_on(blocks: &[Value], d: NaiveDate) -> Vec<Value> {
    let mut out: Vec<Value> = blocks
        .iter()
        .filter(|b| occurs(b, d))
        .map(|b| {
            let mut o = b.clone();
            o["occurrenceDate"] = json!(d.to_string());
            o
        })
        .collect();
    out.sort_by(|a, b| a["startTime"].as_str().cmp(&b["startTime"].as_str()));
    // Flag overlaps so the UI can lay them side by side and warn.
    for i in 0..out.len() {
        let (s, e) = (minutes(out[i]["startTime"].as_str().unwrap_or("")).unwrap_or(0), minutes(out[i]["endTime"].as_str().unwrap_or("")).unwrap_or(0));
        let clash = out.iter().enumerate().any(|(j, o)| {
            j != i && minutes(o["startTime"].as_str().unwrap_or("")).unwrap_or(0) < e && minutes(o["endTime"].as_str().unwrap_or("")).unwrap_or(0) > s
        });
        out[i]["overlaps"] = json!(clash);
    }
    out
}

/// One week (Monday first): blocks, tasks due each day, focus sessions logged, and planned vs actual minutes per goal.
pub fn week(conn: &Connection, start: &str) -> Result<Value> {
    let start = growth::date(start).ok_or_else(|| AppError::Invalid("week start must be YYYY-MM-DD".into()))?;
    let monday = start - Duration::days(start.weekday().num_days_from_monday() as i64);
    let tz = settings::user_tz(conn);
    let blocks = all_blocks(conn)?;
    let mut days = vec![];
    let mut planned_by_goal: std::collections::HashMap<String, i64> = Default::default();
    let mut actual_by_goal: std::collections::HashMap<String, i64> = Default::default();
    let (mut planned_total, mut actual_total) = (0i64, 0i64);
    for i in 0..7 {
        let d = monday + Duration::days(i);
        let ds = d.to_string();
        let occ = occurrences_on(&blocks, d);
        for o in &occ {
            let m = minutes(o["endTime"].as_str().unwrap_or("")).unwrap_or(0) - minutes(o["startTime"].as_str().unwrap_or("")).unwrap_or(0);
            planned_total += m;
            if let Some(g) = o["goalId"].as_str() {
                *planned_by_goal.entry(g.to_string()).or_default() += m;
            }
        }
        let due: Vec<Value> = conn
            .prepare("SELECT id, title, status, priority, remind_at FROM tasks WHERE deleted_at IS NULL AND due_date=?1 ORDER BY status='done', priority DESC")?
            .query_map([&ds], |r| {
                let remind: Option<String> = r.get(4)?;
                Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "status": r.get::<_, String>(2)?, "priority": r.get::<_, String>(3)?,
                           "remindTime": remind.and_then(|u| chrono::DateTime::parse_from_rfc3339(&u).ok()).map(|x| x.with_timezone(&tz)).filter(|x| x.date_naive() == d).map(|x| x.format("%H:%M").to_string()) }))
            })?
            .collect::<std::result::Result<_, _>>()?;
        let from = tz.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap()).earliest().map(|x| x.with_timezone(&chrono::Utc).to_rfc3339()).unwrap_or_default();
        let to = tz.from_local_datetime(&(d + Duration::days(1)).and_hms_opt(0, 0, 0).unwrap()).earliest().map(|x| x.with_timezone(&chrono::Utc).to_rfc3339()).unwrap_or_default();
        let sessions: Vec<Value> = skills::sessions_between(conn, &from, &to)?
            .into_iter()
            .map(|s| {
                let start = chrono::DateTime::parse_from_rfc3339(s["startedAt"].as_str().unwrap_or("")).map(|x| x.with_timezone(&tz).format("%H:%M").to_string()).unwrap_or_default();
                let m = s["actualMinutes"].as_i64().unwrap_or(0);
                actual_total += m;
                if let Some(g) = s["goalId"].as_str() {
                    *actual_by_goal.entry(g.to_string()).or_default() += m;
                }
                json!({ "id": s["id"], "start": start, "minutes": m, "title": s["taskTitle"].as_str().or(s["goalTitle"].as_str()).unwrap_or("Focus"), "goalId": s["goalId"] })
            })
            .collect();
        days.push(json!({ "date": ds, "blocks": occ, "tasks": due, "sessions": sessions }));
    }
    let goal_rows: Vec<Value> = goals::list(conn, None, false)?
        .into_iter()
        .filter_map(|g| {
            let id = g["id"].as_str()?.to_string();
            let planned = planned_by_goal.get(&id).copied().unwrap_or(0);
            let actual = actual_by_goal.get(&id).copied().unwrap_or(0);
            let budget = g["timeBudgetHoursPerWeek"].as_f64().map(|h| (h * 60.0) as i64);
            (planned > 0 || actual > 0 || budget.is_some()).then(|| json!({ "goalId": id, "title": g["title"], "plannedMinutes": planned, "actualMinutes": actual, "budgetMinutes": budget }))
        })
        .collect();
    Ok(json!({ "weekStart": monday.to_string(), "today": tasks::today(conn), "days": days, "plannedMinutes": planned_total, "actualMinutes": actual_total, "goals": goal_rows }))
}

pub fn today(conn: &Connection) -> Result<Vec<Value>> {
    let d = growth::date(&tasks::today(conn)).unwrap();
    Ok(occurrences_on(&all_blocks(conn)?, d))
}

/// Blocks starting within their reminder window right now (claimed once per occurrence).
pub fn due_reminders(conn: &Connection) -> Result<Vec<(String, String)>> {
    let tz = settings::user_tz(conn);
    let local = chrono::Utc::now().with_timezone(&tz);
    let d = local.date_naive();
    let now_min = { use chrono::Timelike; local.hour() as i64 * 60 + local.minute() as i64 };
    let key = format!("blockReminders:{d}");
    let mut sent: Vec<String> = settings::get(conn, &key).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let mut out = vec![];
    for b in occurrences_on(&all_blocks(conn)?, d) {
        let Some(before) = b["remindMinutes"].as_i64() else { continue };
        let start = minutes(b["startTime"].as_str().unwrap_or("")).unwrap_or(0);
        let id = b["id"].as_str().unwrap_or("").to_string();
        if now_min >= start - before && now_min < start + 5 && !sent.contains(&id) {
            let place = b["location"].as_str().filter(|l| !l.is_empty()).map(|l| format!(" · {l}")).unwrap_or_default();
            out.push((format!("🗓 {}", b["title"].as_str().unwrap_or("")), format!("{}–{}{place}", b["startTime"].as_str().unwrap_or(""), b["endTime"].as_str().unwrap_or(""))));
            sent.push(id);
        }
    }
    if !out.is_empty() {
        settings::put(conn, &key, &json!(sent))?;
        conn.execute("DELETE FROM settings WHERE key LIKE 'blockReminders:%' AND key <> ?1", [&key])?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_occurrence_rules() {
        let b = json!({ "recurrence": "weekly", "weekdays": [1, 3], "validFrom": "2026-10-01", "validUntil": "2026-12-31", "skipDates": ["2026-10-21"] });
        assert!(occurs(&b, NaiveDate::from_ymd_opt(2026, 10, 19).unwrap())); // Monday
        assert!(!occurs(&b, NaiveDate::from_ymd_opt(2026, 10, 20).unwrap())); // Tuesday
        assert!(!occurs(&b, NaiveDate::from_ymd_opt(2026, 10, 21).unwrap())); // skipped Wednesday
        assert!(!occurs(&b, NaiveDate::from_ymd_opt(2027, 1, 4).unwrap())); // after the series
    }
}

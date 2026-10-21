//! Today plan and weekly reviews. A review draft is generated from local data only; the user edits and saves it.

use super::{goals, habits, settings, skills, tasks};
use crate::domain::growth;
use crate::domain::{new_id, now};
use crate::error::{invalid, Result};
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub fn today_plan(conn: &Connection) -> Result<Value> {
    let today = tasks::today(conn);
    let mut due = tasks::list(conn, "overdue", None, None, 50)?;
    due.extend(tasks::list(conn, "today", None, None, 100)?);
    let top_goals: Vec<Value> = goals::list(conn, None, false)?.into_iter().filter(|g| g["status"] == "active").take(3).collect();
    let (from, to) = day_bounds_utc(conn, growth::date(&today).unwrap(), 1);
    let focus_today: i64 = skills::sessions_between(conn, &from, &to)?.iter().filter_map(|s| s["actualMinutes"].as_i64()).sum();
    Ok(json!({
        "date": today,
        "tasks": due,
        "habits": habits::due_today(conn)?,
        "goals": top_goals,
        "runningSession": skills::running_session(conn)?,
        "focusMinutesToday": focus_today,
    }))
}

/// [start, start + days) of local calendar days as UTC RFC3339 strings.
fn day_bounds_utc(conn: &Connection, start: NaiveDate, days: i64) -> (String, String) {
    let tz = settings::user_tz(conn);
    let at = |d: NaiveDate| {
        tz.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .map(|x| x.with_timezone(&Utc))
            .unwrap_or_else(|| Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    };
    (at(start), at(start + Duration::days(days)))
}

/// "2026-W43" → Monday of that ISO week.
pub fn week_start(key: &str) -> Option<NaiveDate> {
    let (y, w) = key.split_once("-W")?;
    NaiveDate::from_isoywd_opt(y.parse().ok()?, w.parse().ok()?, chrono::Weekday::Mon)
}

pub fn week_key(d: NaiveDate) -> String {
    let w = d.iso_week();
    format!("{}-W{:02}", w.year(), w.week())
}

pub fn current_week(conn: &Connection) -> String {
    week_key(growth::date(&tasks::today(conn)).unwrap())
}

/// Fresh summary of one week from tasks, milestones, evidence, habits, focus time and spending.
pub fn generate(conn: &Connection, key: &str) -> Result<Value> {
    let start = week_start(key).ok_or_else(|| crate::error::AppError::Invalid("week must look like 2026-W43".into()))?;
    let end = start + Duration::days(6);
    let (from, to) = day_bounds_utc(conn, start, 7);
    let (s, e) = (start.to_string(), end.to_string());

    let completed: Vec<Value> = conn
        .prepare("SELECT id, title FROM tasks WHERE deleted_at IS NULL AND status='done' AND completed_at >= ?1 AND completed_at < ?2 ORDER BY completed_at")?
        .query_map([&from, &to], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)? })))?
        .collect::<std::result::Result<_, _>>()?;
    let slipped: Vec<Value> = conn
        .prepare("SELECT id, title, due_date FROM tasks WHERE deleted_at IS NULL AND status<>'done' AND due_date >= ?1 AND due_date <= ?2 ORDER BY due_date")?
        .query_map([&s, &e], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "dueDate": r.get::<_, String>(2)? })))?
        .collect::<std::result::Result<_, _>>()?;
    let milestones_met: Vec<Value> = conn
        .prepare("SELECT m.id, m.title, g.title FROM milestones m JOIN goals g ON g.id=m.goal_id WHERE m.status='met' AND m.updated_at >= ?1 AND m.updated_at < ?2")?
        .query_map([&from, &to], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "goal": r.get::<_, String>(2)? })))?
        .collect::<std::result::Result<_, _>>()?;
    let evidence = skills::evidence_between(conn, &s, &e)?;
    let habit_rows = habits::consistency_between(conn, start, end.min(growth::date(&tasks::today(conn)).unwrap()))?;
    let sessions = skills::sessions_between(conn, &from, &to)?;
    let focus_total: i64 = sessions.iter().filter_map(|x| x["actualMinutes"].as_i64()).sum();

    let goal_rows: Vec<Value> = goals::list(conn, None, false)?
        .into_iter()
        .map(|g| {
            let minutes: i64 = sessions.iter().filter(|x| x["goalId"] == g["id"]).filter_map(|x| x["actualMinutes"].as_i64()).sum();
            json!({
                "id": g["id"], "title": g["title"], "status": g["status"], "health": g["metrics"]["health"]["state"],
                "preparation": g["metrics"]["preparation"], "taskCompletion": g["metrics"]["taskCompletion"],
                "focusMinutes": minutes, "plannedMinutes": g["timeBudgetHoursPerWeek"].as_f64().map(|h| (h * 60.0) as i64),
            })
        })
        .collect();

    // Spending: totals only, per currency, from local data.
    let spend: Vec<Value> = conn
        .prepare(
            "SELECT t.currency, COALESCE(c.name,'Uncategorised'), SUM(t.amount_minor) FROM transactions t LEFT JOIN financial_categories c ON c.id=t.category_id
             WHERE t.deleted_at IS NULL AND t.type='expense' AND t.occurred_on >= ?1 AND t.occurred_on <= ?2 GROUP BY t.currency, c.id ORDER BY 3 DESC",
        )?
        .query_map([&s, &e], |r| Ok(json!({ "currency": r.get::<_, String>(0)?, "category": r.get::<_, String>(1)?, "totalMinor": r.get::<_, i64>(2)? })))?
        .collect::<std::result::Result<_, _>>()?;

    // Progress delta against the previous saved review's snapshot.
    let prev_key = week_key(start - Duration::days(7));
    let prev: Option<Value> = conn
        .query_row("SELECT summary_json FROM reviews WHERE kind='weekly' AND period_key=?1", [&prev_key], |r| r.get::<_, String>(0))
        .optional()?
        .and_then(|s| serde_json::from_str(&s).ok());
    let goal_rows: Vec<Value> = goal_rows
        .into_iter()
        .map(|mut g| {
            let before = prev.as_ref().and_then(|p| p["goals"].as_array()?.iter().find(|x| x["id"] == g["id"])?.get("preparation")?.as_f64());
            if let (Some(b), Some(n)) = (before, g["preparation"].as_f64()) {
                g["preparationDelta"] = json!(n - b);
            }
            g
        })
        .collect();

    Ok(json!({
        "period": key, "start": s, "end": e,
        "completedTasks": completed, "slippedTasks": slipped, "milestonesMet": milestones_met,
        "evidence": evidence.iter().map(|x| json!({ "id": x["id"], "title": x["title"], "skill": x["skillName"], "level": x["level"], "type": x["evidenceType"], "observedAt": x["observedAt"] })).collect::<Vec<_>>(),
        "habits": habit_rows, "focusMinutes": focus_total, "goals": goal_rows, "spending": spend,
        "previousReview": prev.is_some(),
    }))
}

pub fn get(conn: &Connection, key: &str) -> Result<Value> {
    let saved: Option<Value> = conn
        .query_row(
            "SELECT reflection, learnings, next_commitments, summary_json, updated_at FROM reviews WHERE kind='weekly' AND period_key=?1",
            [key],
            |r| Ok(json!({ "reflection": r.get::<_, String>(0)?, "learnings": r.get::<_, String>(1)?, "nextCommitments": r.get::<_, String>(2)?, "summary": serde_json::from_str::<Value>(&r.get::<_, String>(3)?).unwrap_or(Value::Null), "updatedAt": r.get::<_, String>(4)? })),
        )
        .optional()?;
    Ok(json!({ "period": key, "draft": generate(conn, key)?, "saved": saved }))
}

/// Saves the user's review with a snapshot of the generated summary. Optionally marks active goals as reviewed.
pub fn save(conn: &Connection, key: &str, p: &Value) -> Result<Value> {
    if week_start(key).is_none() {
        return invalid("week must look like 2026-W43");
    }
    let summary = generate(conn, key)?;
    let f = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    conn.execute(
        "INSERT INTO reviews(id, kind, period_key, summary_json, reflection, learnings, next_commitments, created_at, updated_at) VALUES (?1,'weekly',?2,?3,?4,?5,?6,?7,?7)
         ON CONFLICT(kind, period_key) DO UPDATE SET summary_json=excluded.summary_json, reflection=excluded.reflection, learnings=excluded.learnings,
           next_commitments=excluded.next_commitments, updated_at=excluded.updated_at",
        params![new_id(), key, summary.to_string(), f("reflection"), f("learnings"), f("nextCommitments"), now()],
    )?;
    if p.get("markGoalsReviewed").and_then(|v| v.as_bool()) == Some(true) {
        conn.execute("UPDATE goals SET last_reviewed_at=?1 WHERE status='active' AND deleted_at IS NULL", [tasks::today(conn)])?;
    }
    get(conn, key)
}

pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    let rows = conn
        .prepare("SELECT period_key, updated_at FROM reviews WHERE kind='weekly' ORDER BY period_key DESC LIMIT 52")?
        .query_map([], |r| Ok(json!({ "period": r.get::<_, String>(0)?, "updatedAt": r.get::<_, String>(1)? })))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_weeks_round_trip() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let k = week_key(d);
        assert_eq!(k, "2026-W43");
        assert_eq!(week_start(&k).unwrap(), NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
    }
}

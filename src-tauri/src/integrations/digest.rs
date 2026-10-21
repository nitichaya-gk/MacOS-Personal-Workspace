//! What should be announced right now: due and missed reminders, the daily summary, habit reminders,
//! scheduled automations and queued automation notifications. Shared by the app's scheduler and the
//! headless background agent; every item is claimed in the database, so the two never announce twice.

use crate::application::{automations, habits, settings, tasks};
use rusqlite::Connection;
use serde_json::{json, Value};

/// Reminders older than this when first seen are summarised instead of fired one by one.
pub const GRACE_SECS: i64 = 120;

pub struct Note {
    pub title: String,
    pub body: String,
    /// Event for the app window (name, payload); the agent ignores it.
    pub event: Option<(&'static str, Value)>,
}

fn note(title: impl Into<String>, body: impl Into<String>, event: Option<(&'static str, Value)>) -> Note {
    Note { title: title.into(), body: body.into(), event }
}

pub fn collect(conn: &Connection) -> Vec<Note> {
    let mut out = vec![];
    match tasks::take_due_reminders(conn, GRACE_SECS) {
        Ok(batch) => {
            for r in &batch.fire {
                let body = r.task.due_date.as_ref().map(|d| format!("Due {d}")).unwrap_or_else(|| "Reminder".into());
                out.push(note(format!("⏰ {}", r.task.title), body, Some(("reminder-fired", json!({ "taskId": r.task.id, "title": r.task.title })))));
            }
            if !batch.missed.is_empty() {
                let n = batch.missed.len();
                let first = &batch.missed[0].task.title;
                let body = if n == 1 { first.clone() } else { format!("{first} and {} more", n - 1) };
                out.push(note(format!("You missed {n} reminder{}", if n == 1 { "" } else { "s" }), body, Some(("reminders-missed", json!({ "count": n, "taskId": batch.missed[0].task.id })))));
            }
        }
        Err(e) => eprintln!("reminder check failed: {e}"),
    }
    if let Some(n) = daily_summary(conn) {
        out.push(n);
    }
    out.extend(habit_reminders(conn));
    match crate::application::timetable::due_reminders(conn) {
        Ok(items) => out.extend(items.into_iter().map(|(t, b)| note(t, b, Some(("block-starting", json!({})))))),
        Err(e) => eprintln!("timetable reminders failed: {e}"),
    }
    if let Err(e) = automations::tick_schedules(conn) {
        eprintln!("scheduled automations failed: {e}");
    }
    if let Ok(items) = automations::take_outbox(conn) {
        for (title, body, target) in items {
            out.push(note(title.clone(), body, Some(("automation-notified", json!({ "title": title, "targetId": target })))));
        }
    }
    out
}

fn daily_summary(conn: &Connection) -> Option<Note> {
    let cfg = settings::get(conn, "dailySummary").ok()?;
    if cfg.get("enabled").and_then(|e| e.as_bool()) != Some(true) {
        return None;
    }
    let local = chrono::Utc::now().with_timezone(&settings::user_tz(conn));
    let today = local.format("%Y-%m-%d").to_string();
    if local.format("%H:%M").to_string().as_str() < cfg.get("time").and_then(|t| t.as_str()).unwrap_or("08:30") {
        return None;
    }
    // Claim today's summary atomically so app and agent never both send it.
    let claimed = conn
        .execute(
            "INSERT INTO settings(key, value) VALUES ('lastSummaryDate', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value WHERE settings.value <> excluded.value",
            [json!(today).to_string()],
        )
        .ok()?;
    if claimed == 0 {
        return None;
    }
    let c = tasks::counts(conn).ok()?;
    let (due, overdue) = (c["today"].as_i64().unwrap_or(0), c["overdue"].as_i64().unwrap_or(0));
    let body = match (due, overdue) {
        (0, 0) => "Nothing due today.".to_string(),
        (d, 0) => format!("{d} task{} due today.", if d == 1 { "" } else { "s" }),
        (d, o) => format!("{d} due today · {o} overdue."),
    };
    Some(note("Today", body, Some(("daily-summary", json!({ "today": due, "overdue": overdue })))))
}

/// One reminder per habit per day at its reminder time, only if it is due and not yet logged.
fn habit_reminders(conn: &Connection) -> Vec<Note> {
    let local = chrono::Utc::now().with_timezone(&settings::user_tz(conn));
    let today = local.format("%Y-%m-%d").to_string();
    let hm = local.format("%H:%M").to_string();
    let key = format!("habitReminders:{today}");
    let mut sent: Vec<String> = settings::get(conn, &key).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let Ok(due) = habits::due_today(conn) else { return vec![] };
    let mut out = vec![];
    for h in due {
        let id = h["id"].as_str().unwrap_or("").to_string();
        let Some(t) = h["reminderTime"].as_str() else { continue };
        if t > hm.as_str() || sent.contains(&id) || !h["stats"]["todayStatus"].is_null() {
            continue;
        }
        out.push(note(format!("🔁 {}", h["title"].as_str().unwrap_or("")), "Habit due today", None));
        sent.push(id);
    }
    if !out.is_empty() {
        let _ = settings::put(conn, &key, &json!(sent));
        let _ = conn.execute("DELETE FROM settings WHERE key LIKE 'habitReminders:%' AND key <> ?1", [&key]);
    }
    out
}

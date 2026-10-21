//! Background scheduler (runs while the app is open): task reminders, the daily summary,
//! catching up on reminders missed while the app was closed or the Mac was asleep, and daily auto-backup.

use crate::application::{settings, tasks};
use crate::infrastructure::backup;
use crate::AppState;
use serde_json::json;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const TICK: Duration = Duration::from_secs(15);
/// Reminders older than this when first seen are summarised instead of fired one by one.
const GRACE_SECS: i64 = 120;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        tick(&app);
        std::thread::sleep(TICK);
    });
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        eprintln!("notification failed: {e}");
    }
}

fn tick(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(ws) = state.ws.lock() else { return };
    let conn = &ws.conn;

    match tasks::take_due_reminders(conn, GRACE_SECS) {
        Ok(batch) => {
            for r in &batch.fire {
                let body = match &r.task.due_date {
                    Some(d) => format!("Due {d}"),
                    None => "Reminder".to_string(),
                };
                notify(app, &format!("⏰ {}", r.task.title), &body);
                let _ = app.emit("reminder-fired", json!({ "taskId": r.task.id, "title": r.task.title }));
            }
            if !batch.missed.is_empty() {
                let n = batch.missed.len();
                let first = &batch.missed[0].task.title;
                let body = if n == 1 { first.clone() } else { format!("{first} and {} more", n - 1) };
                notify(app, &format!("You missed {n} reminder{}", if n == 1 { "" } else { "s" }), &body);
                let _ = app.emit("reminders-missed", json!({ "count": n, "taskId": batch.missed[0].task.id }));
            }
        }
        Err(e) => eprintln!("reminder check failed: {e}"),
    }

    daily_summary(app, conn);
    habit_reminders(app, conn);
    auto_backup(&ws);
}

fn daily_summary(app: &AppHandle, conn: &rusqlite::Connection) {
    let Ok(cfg) = settings::get(conn, "dailySummary") else { return };
    if cfg.get("enabled").and_then(|e| e.as_bool()) != Some(true) {
        return;
    }
    let tz = settings::user_tz(conn);
    let local = chrono::Utc::now().with_timezone(&tz);
    let today = local.format("%Y-%m-%d").to_string();
    let time = cfg.get("time").and_then(|t| t.as_str()).unwrap_or("08:30");
    if local.format("%H:%M").to_string().as_str() < time {
        return;
    }
    let last = settings::get(conn, "lastSummaryDate").ok().and_then(|v| v.as_str().map(String::from));
    if last.as_deref() == Some(today.as_str()) {
        return;
    }
    let _ = settings::put(conn, "lastSummaryDate", &json!(today));
    let Ok(c) = tasks::counts(conn) else { return };
    let (due, overdue) = (c["today"].as_i64().unwrap_or(0), c["overdue"].as_i64().unwrap_or(0));
    let body = match (due, overdue) {
        (0, 0) => "Nothing due today.".to_string(),
        (d, 0) => format!("{d} task{} due today.", if d == 1 { "" } else { "s" }),
        (d, o) => format!("{d} due today · {o} overdue."),
    };
    notify(app, "Today", &body);
    let _ = app.emit("daily-summary", json!({ "today": due, "overdue": overdue }));
}

fn auto_backup(ws: &crate::application::workspace::Workspace) {
    let conn = &ws.conn;
    let Ok(cfg) = settings::get(conn, "backup") else { return };
    if cfg.get("auto").and_then(|a| a.as_bool()) != Some(true) {
        return;
    }
    let last = settings::get(conn, "lastAutoBackupAt").ok().and_then(|v| v.as_str().map(String::from));
    let due = match last.and_then(|l| chrono::DateTime::parse_from_rfc3339(&l).ok()) {
        Some(t) => chrono::Utc::now().signed_duration_since(t) > chrono::Duration::hours(24),
        None => true,
    };
    if !due {
        return;
    }
    let Ok(manifest) = ws.manifest() else { return };
    match backup::create(conn, &ws.root, &manifest, "auto") {
        Ok(_) => {
            let _ = settings::put(conn, "lastAutoBackupAt", &json!(crate::domain::now()));
            let keep = cfg.get("keep").and_then(|k| k.as_u64()).unwrap_or(10) as usize;
            let _ = backup::prune(&ws.root, keep);
        }
        Err(e) => eprintln!("auto backup failed: {e}"),
    }
}

/// One reminder per habit per day at its reminder time, only if it is due and not yet logged.
fn habit_reminders(app: &AppHandle, conn: &rusqlite::Connection) {
    let tz = settings::user_tz(conn);
    let local = chrono::Utc::now().with_timezone(&tz);
    let today = local.format("%Y-%m-%d").to_string();
    let hm = local.format("%H:%M").to_string();
    let key = format!("habitReminders:{today}");
    let mut sent: Vec<String> = settings::get(conn, &key).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let Ok(due) = crate::application::habits::due_today(conn) else { return };
    let mut changed = false;
    for h in due {
        let id = h["id"].as_str().unwrap_or("").to_string();
        let Some(t) = h["reminderTime"].as_str() else { continue };
        if t > hm.as_str() || sent.contains(&id) || !h["stats"]["todayStatus"].is_null() {
            continue;
        }
        notify(app, &format!("🔁 {}", h["title"].as_str().unwrap_or("")), "Habit due today");
        sent.push(id);
        changed = true;
    }
    if changed {
        let _ = settings::put(conn, &key, &json!(sent));
        let _ = conn.execute("DELETE FROM settings WHERE key LIKE 'habitReminders:%' AND key <> ?1", [&key]);
    }
}

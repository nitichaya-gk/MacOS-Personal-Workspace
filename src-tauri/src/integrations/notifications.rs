//! In-app scheduler (runs while the app is open): delivers whatever `digest::collect` finds as macOS
//! notifications plus window events, and makes the daily auto-backup.

use super::digest;
use crate::application::settings;
use crate::infrastructure::backup;
use crate::AppState;
use serde_json::json;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const TICK: Duration = Duration::from_secs(15);

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        tick(&app);
        std::thread::sleep(TICK);
    });
}

fn tick(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(ws) = state.ws.lock() else { return };
    for n in digest::collect(&ws.conn) {
        if let Err(e) = app.notification().builder().title(&n.title).body(&n.body).show() {
            eprintln!("notification failed: {e}");
        }
        if let Some((name, payload)) = n.event {
            let _ = app.emit(name, payload);
        }
    }
    auto_backup(&ws);
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

//! Workspace settings stored as JSON values in the `settings` table.

use crate::error::{invalid, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};

pub fn defaults() -> Map<String, Value> {
    let v = json!({
        "language": "system",
        "timezone": "system",
        "dailySummary": { "enabled": false, "time": "08:30" },
        "backup": { "auto": true, "keep": 10 },
        "mcp": { "enabled": false, "scope": "selected", "allowedIds": [] },
        "notificationsAsked": false
    });
    v.as_object().cloned().unwrap()
}

/// Keys the UI may change. Internal bookkeeping keys are written only by services.
const USER_KEYS: &[&str] = &["language", "timezone", "dailySummary", "backup", "mcp", "notificationsAsked"];

pub fn get(conn: &Connection, key: &str) -> Result<Value> {
    let stored: Option<String> = conn.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0)).optional()?;
    Ok(match stored {
        Some(s) => serde_json::from_str(&s)?,
        None => defaults().get(key).cloned().unwrap_or(Value::Null),
    })
}

pub fn put(conn: &Connection, key: &str, value: &Value) -> Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, serde_json::to_string(value)?],
    )?;
    Ok(())
}

pub fn all(conn: &Connection) -> Result<Value> {
    let mut out = defaults();
    for key in USER_KEYS {
        out.insert(key.to_string(), get(conn, key)?);
    }
    out.insert("effectiveTimezone".into(), json!(user_tz(conn).name()));
    Ok(Value::Object(out))
}

pub fn set_user(conn: &Connection, key: &str, value: &Value) -> Result<Value> {
    if !USER_KEYS.contains(&key) {
        return invalid(format!("unknown setting {key}"));
    }
    match key {
        "timezone" => {
            let tz = value.as_str().unwrap_or("");
            if tz != "system" && tz.parse::<chrono_tz::Tz>().is_err() {
                return invalid("unknown time zone");
            }
        }
        "language" => {
            if !matches!(value.as_str(), Some("system" | "en" | "th")) {
                return invalid("language must be system, en or th");
            }
        }
        "dailySummary" => {
            let t = value.get("time").and_then(|t| t.as_str()).unwrap_or("");
            if chrono::NaiveTime::parse_from_str(t, "%H:%M").is_err() {
                return invalid("time must be HH:MM");
            }
        }
        "backup" => {
            let keep = value.get("keep").and_then(|k| k.as_u64()).unwrap_or(0);
            if !(1..=100).contains(&keep) {
                return invalid("keep must be between 1 and 100");
            }
        }
        _ => {}
    }
    put(conn, key, value)?;
    all(conn)
}

pub fn system_tz() -> chrono_tz::Tz {
    iana_time_zone::get_timezone().ok().and_then(|n| n.parse().ok()).unwrap_or(chrono_tz::UTC)
}

pub fn user_tz(conn: &Connection) -> chrono_tz::Tz {
    match get(conn, "timezone").ok().as_ref().and_then(|v| v.as_str()) {
        Some("system") | None => system_tz(),
        Some(name) => name.parse().unwrap_or_else(|_| system_tz()),
    }
}

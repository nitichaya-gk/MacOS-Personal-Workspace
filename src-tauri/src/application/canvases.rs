//! Diagram / Wireframe canvases. Objects are stored as editable element JSON, one row per object.

use super::pages;
use super::workspace::Workspace;
use crate::domain::{now, parse_workspace_link, truncate_chars};
use crate::error::{invalid, AppError, Result};
use crate::infrastructure::filesystem;
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub fn load(ws: &Workspace, id: &str) -> Result<Value> {
    let conn = &ws.conn;
    let meta = pages::get_meta(conn, id)?;
    let (kind, app_state): (String, String) = conn
        .query_row("SELECT kind, app_state FROM canvases WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?
        .ok_or_else(|| AppError::NotFound("canvas".into()))?;
    let elements: Vec<Value> = conn
        .prepare("SELECT data FROM canvas_objects WHERE canvas_id=?1 ORDER BY position")?
        .query_map([id], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok().and_then(|s| serde_json::from_str(&s).ok()))
        .collect();
    let mut files = vec![];
    for el in &elements {
        if let Some(fid) = el.get("fileId").and_then(|f| f.as_str()) {
            if files.iter().any(|f: &Value| f["id"] == fid) {
                continue;
            }
            if let Some((asset_id, mime)) = find_file(conn, fid)? {
                if let Ok(bytes) = std::fs::read(filesystem::asset_path(&ws.root, &asset_id)?) {
                    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
                    files.push(json!({ "id": fid, "mimeType": mime, "dataURL": format!("data:{mime};base64,{data}"), "created": 0 }));
                }
            }
        }
    }
    Ok(json!({
        "id": id,
        "title": meta.title,
        "kind": kind,
        "elements": elements,
        "appState": serde_json::from_str::<Value>(&app_state).unwrap_or(json!({})),
        "files": files,
        "updatedAt": meta.updated_at,
    }))
}

fn find_file(conn: &Connection, file_id: &str) -> Result<Option<(String, String)>> {
    filesystem::check_asset_id(file_id)?;
    Ok(conn
        .query_row("SELECT id, mime FROM assets WHERE id = ?1 OR id LIKE ?1 || '.%' LIMIT 1", [file_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?)
}

pub fn save(conn: &Connection, id: &str, elements: &[Value], app_state: &Value) -> Result<String> {
    let meta = pages::get_meta(conn, id)?;
    if meta.kind != "canvas" {
        return invalid("not a canvas");
    }
    let ts = now();
    let tx = crate::infrastructure::database::begin(conn)?;
    tx.execute("DELETE FROM canvas_objects WHERE canvas_id=?1", [id])?;
    let mut targets = vec![];
    {
        let mut ins = tx.prepare("INSERT OR REPLACE INTO canvas_objects(canvas_id, id, position, type, data) VALUES (?1,?2,?3,?4,?5)")?;
        for (i, el) in elements.iter().enumerate() {
            if el.get("isDeleted").and_then(|d| d.as_bool()) == Some(true) {
                continue;
            }
            let Some(eid) = el.get("id").and_then(|v| v.as_str()) else { continue };
            let etype = el.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
            ins.execute(params![id, eid, i as i64, etype, el.to_string()])?;
            if let Some(target) = el.get("link").and_then(|l| l.as_str()).and_then(parse_workspace_link) {
                let ctx = el.get("text").and_then(|t| t.as_str()).unwrap_or(etype);
                targets.push((target, truncate_chars(ctx, 120)));
            }
        }
    }
    pages::set_links(&tx, id, "canvas", &targets)?;
    tx.execute("UPDATE canvases SET app_state=?2, updated_at=?3 WHERE id=?1", params![id, app_state.to_string(), ts])?;
    tx.execute("UPDATE pages SET updated_at=?2 WHERE id=?1", params![id, ts])?;
    tx.commit()?;
    pages::index(conn, id)?;
    Ok(ts)
}

/// Stores an image placed on a canvas (Excalidraw "binary file") under assets/.
pub fn save_file(ws: &Workspace, canvas_id: &str, file_id: &str, mime: &str, data_url: &str) -> Result<()> {
    filesystem::check_asset_id(file_id)?;
    if find_file(&ws.conn, file_id)?.is_some() {
        return Ok(());
    }
    let b64 = data_url.split_once(',').map(|(_, d)| d).unwrap_or(data_url);
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|_| AppError::Invalid("invalid image data".into()))?;
    let asset_id = format!("{file_id}.{}", filesystem::ext_for_mime(mime));
    filesystem::atomic_write(&filesystem::asset_path(&ws.root, &asset_id)?, &bytes)?;
    ws.conn.execute(
        "INSERT OR IGNORE INTO assets(id, file_name, mime, size, page_id, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![asset_id, asset_id, mime, bytes.len() as i64, canvas_id, now()],
    )?;
    Ok(())
}

pub fn canvas_text(conn: &Connection, id: &str) -> Result<String> {
    let rows: Vec<String> = conn
        .prepare("SELECT data FROM canvas_objects WHERE canvas_id=?1 ORDER BY position")?
        .query_map([id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let mut parts = vec![];
    for raw in rows {
        let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        for key in ["text", "name"] {
            if let Some(s) = v.get(key).and_then(|t| t.as_str()).filter(|s| !s.is_empty()) {
                parts.push(s.to_string());
            }
        }
    }
    Ok(parts.join("\n"))
}

/// Compact description of a canvas for MCP clients: shapes, text, connections and links.
pub fn summary(conn: &Connection, id: &str) -> Result<Value> {
    let meta = pages::get_meta(conn, id)?;
    let kind: String = conn.query_row("SELECT kind FROM canvases WHERE id=?1", [id], |r| r.get(0))?;
    let rows: Vec<String> = conn
        .prepare("SELECT data FROM canvas_objects WHERE canvas_id=?1 ORDER BY position")?
        .query_map([id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let els: Vec<Value> = rows.iter().filter_map(|r| serde_json::from_str(r).ok()).collect();
    let text_of = |eid: &str| -> Option<String> {
        els.iter()
            .find(|e| e.get("containerId").and_then(|c| c.as_str()) == Some(eid))
            .and_then(|t| t.get("text").and_then(|s| s.as_str()).map(String::from))
    };
    let mut objects = vec![];
    let mut connections = vec![];
    for e in &els {
        let etype = e["type"].as_str().unwrap_or("");
        let eid = e["id"].as_str().unwrap_or("");
        if etype == "arrow" || etype == "line" {
            let from = e.pointer("/startBinding/elementId").and_then(|v| v.as_str());
            let to = e.pointer("/endBinding/elementId").and_then(|v| v.as_str());
            if from.is_some() || to.is_some() {
                connections.push(json!({ "id": eid, "from": from, "to": to, "label": text_of(eid) }));
                continue;
            }
        }
        if etype == "text" && e.get("containerId").and_then(|c| c.as_str()).is_some() {
            continue;
        }
        objects.push(json!({
            "id": eid,
            "type": etype,
            "text": e.get("text").and_then(|t| t.as_str()).map(String::from).or_else(|| text_of(eid)),
            "name": e.get("name"),
            "x": e["x"], "y": e["y"], "width": e["width"], "height": e["height"],
            "frameId": e.get("frameId"),
            "link": e.get("link"),
        }));
    }
    Ok(json!({ "id": id, "title": meta.title, "kind": kind, "updatedAt": meta.updated_at, "objects": objects, "connections": connections }))
}

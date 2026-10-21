//! Attachments stored in <workspace>/assets and referenced by asset id (wsasset://localhost/<id>).

use super::workspace::Workspace;
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use crate::infrastructure::filesystem;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

pub const MAX_ASSET_BYTES: usize = 512 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    pub id: String,
    pub file_name: String,
    pub mime: String,
    pub size: i64,
    pub page_id: Option<String>,
    pub created_at: String,
    pub url: String,
}

pub fn url_for(id: &str) -> String {
    format!("wsasset://localhost/{id}")
}

pub fn upload(ws: &Workspace, file_name: &str, mime: &str, page_id: Option<&str>, bytes: &[u8]) -> Result<AssetInfo> {
    if bytes.len() > MAX_ASSET_BYTES {
        return invalid("file is larger than 512 MB");
    }
    let ext = filesystem::safe_extension(file_name);
    let id = format!("{}.{ext}", new_id());
    let mime = if mime.is_empty() || mime == "application/octet-stream" { filesystem::mime_for_ext(&ext) } else { mime };
    filesystem::atomic_write(&filesystem::asset_path(&ws.root, &id)?, bytes)?;
    let page_id = page_id.filter(|p| super::pages::exists(&ws.conn, p).unwrap_or(false));
    ws.conn.execute(
        "INSERT INTO assets(id, file_name, mime, size, page_id, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![id, file_name, mime, bytes.len() as i64, page_id, now()],
    )?;
    get(ws, &id)
}

pub fn get(ws: &Workspace, id: &str) -> Result<AssetInfo> {
    filesystem::check_asset_id(id)?;
    ws.conn
        .query_row("SELECT id, file_name, mime, size, page_id, created_at FROM assets WHERE id=?1", [id], |r| {
            let id: String = r.get(0)?;
            Ok(AssetInfo { url: url_for(&id), id, file_name: r.get(1)?, mime: r.get(2)?, size: r.get(3)?, page_id: r.get(4)?, created_at: r.get(5)? })
        })
        .optional()?
        .ok_or_else(|| AppError::NotFound("asset".into()))
}

/// Assets attached to any page inside the given page's subtree (e.g. a project).
pub fn list_for(ws: &Workspace, root_page_id: &str) -> Result<Vec<AssetInfo>> {
    let ids = super::pages::subtree_ids(&ws.conn, root_page_id)?;
    let mut out = vec![];
    let mut stmt = ws.conn.prepare("SELECT id FROM assets WHERE page_id=?1 ORDER BY created_at DESC")?;
    for pid in ids {
        let aids: Vec<String> = stmt.query_map([&pid], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        for a in aids {
            out.push(get(ws, &a)?);
        }
    }
    Ok(out)
}

/// Bytes + mime for the wsasset:// protocol handler.
pub fn read(root: &std::path::Path, id: &str) -> Result<(Vec<u8>, String)> {
    let path = filesystem::asset_path(root, id)?;
    let bytes = std::fs::read(&path)?;
    let ext = filesystem::safe_extension(id);
    Ok((bytes, filesystem::mime_for_ext(&ext).to_string()))
}

//! Workspace location, opening/creating a workspace folder and the manifest.

use crate::error::{AppError, Result};
use crate::infrastructure::{database, filesystem};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

pub const APP_IDENTIFIER: &str = "com.personalworkspace.desktop";
pub const MANIFEST_FORMAT: &str = "personal-workspace";

pub fn app_support_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PW_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Library/Application Support").join(APP_IDENTIFIER)
}

pub fn logs_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Library/Logs").join(APP_IDENTIFIER)
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub workspace_root: Option<String>,
    pub recent_workspaces: Vec<String>,
}

pub fn load_config() -> AppConfig {
    std::fs::read(app_support_dir().join("config.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_config(cfg: &AppConfig) -> Result<()> {
    filesystem::atomic_write(&app_support_dir().join("config.json"), &serde_json::to_vec_pretty(cfg)?)
}

pub fn default_workspace_root() -> PathBuf {
    app_support_dir().join("Workspace")
}

/// `PW_WORKSPACE` env → config.json → default under Application Support.
pub fn resolve_workspace_root() -> PathBuf {
    if let Ok(p) = std::env::var("PW_WORKSPACE") {
        return PathBuf::from(p);
    }
    load_config().workspace_root.map(PathBuf::from).unwrap_or_else(default_workspace_root)
}

pub fn remember_workspace(root: &Path) -> Result<()> {
    let mut cfg = load_config();
    let s = root.to_string_lossy().to_string();
    cfg.workspace_root = Some(s.clone());
    cfg.recent_workspaces.retain(|r| r != &s);
    cfg.recent_workspaces.insert(0, s);
    cfg.recent_workspaces.truncate(8);
    save_config(&cfg)
}

pub struct Workspace {
    pub conn: Connection,
    pub root: PathBuf,
    pub id: String,
    pub name: String,
}

impl Workspace {
    pub fn open(root: &Path) -> Result<Self> {
        for sub in ["", "assets", "previews", "backups"] {
            std::fs::create_dir_all(root.join(sub))?;
        }
        let conn = database::open(&root.join("workspace.sqlite"))?;
        let existing: Option<(String, String)> = conn
            .query_row("SELECT id, name FROM workspaces LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .ok();
        let (id, name) = match existing {
            Some(w) => w,
            None => super::seed::seed(&conn)?,
        };
        let ws = Workspace { conn, root: root.to_path_buf(), id, name };
        ws.write_manifest()?;
        Ok(ws)
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("workspace.sqlite")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn manifest(&self) -> Result<serde_json::Value> {
        let created: String = self.conn.query_row("SELECT created_at FROM workspaces WHERE id=?1", [&self.id], |r| r.get(0))?;
        Ok(json!({
            "format": MANIFEST_FORMAT,
            "formatVersion": 1,
            "schemaVersion": database::schema_version(&self.conn)?,
            "workspaceId": self.id,
            "name": self.name,
            "createdAt": created,
            "appVersion": env!("CARGO_PKG_VERSION"),
        }))
    }

    fn write_manifest(&self) -> Result<()> {
        filesystem::atomic_write(&self.root.join("manifest.json"), &serde_json::to_vec_pretty(&self.manifest()?)?)
    }

    pub fn info(&self) -> Result<serde_json::Value> {
        let count = |sql: &str| -> Result<i64> { Ok(self.conn.query_row(sql, [], |r| r.get(0))?) };
        Ok(json!({
            "id": self.id,
            "name": self.name,
            "root": self.root.to_string_lossy(),
            "schemaVersion": database::schema_version(&self.conn)?,
            "pages": count("SELECT COUNT(*) FROM pages WHERE deleted_at IS NULL")?,
            "tasks": count("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL")?,
            "assetsBytes": filesystem::dir_size(&self.assets_dir()),
            "recentWorkspaces": load_config().recent_workspaces,
            "defaultRoot": default_workspace_root().to_string_lossy(),
            "appSupportDir": app_support_dir().to_string_lossy(),
            "logsDir": logs_dir().to_string_lossy(),
        }))
    }

    pub fn rename(&mut self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Invalid("name is required".into()));
        }
        self.conn.execute("UPDATE workspaces SET name=?1 WHERE id=?2", [name, &self.id])?;
        self.name = name.to_string();
        self.write_manifest()
    }
}

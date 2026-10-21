//! Consistent backups (SQLite VACUUM INTO + assets copy + manifest), retention, restore and integrity checks.

use crate::error::{invalid, AppError, Result};
use crate::infrastructure::{database, filesystem};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn backups_dir(root: &Path) -> PathBuf {
    root.join("backups")
}

/// Creates `<root>/backups/<timestamp>/`. The folder appears under its final name only once complete.
pub fn create(conn: &Connection, root: &Path, manifest: &Value, label: &str) -> Result<Value> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let name = if label.is_empty() { stamp } else { format!("{stamp}-{label}") };
    let dir = backups_dir(root);
    fs::create_dir_all(&dir)?;
    let partial = dir.join(format!(".{name}.partial"));
    let final_dir = dir.join(&name);
    write_snapshot(conn, root, manifest, &partial).inspect_err(|_| {
        let _ = fs::remove_dir_all(&partial);
    })?;
    fs::rename(&partial, &final_dir)?;
    info(&final_dir)
}

fn write_snapshot(conn: &Connection, root: &Path, manifest: &Value, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    // VACUUM INTO gives a transactionally consistent copy even while the app keeps writing.
    conn.execute("VACUUM INTO ?1", [dest.join("workspace.sqlite").to_string_lossy().as_ref()])?;
    filesystem::copy_dir_all(&root.join("assets"), &dest.join("assets"))?;
    let mut m = manifest.clone();
    m["backupCreatedAt"] = json!(crate::domain::now());
    filesystem::atomic_write(&dest.join("manifest.json"), &serde_json::to_vec_pretty(&m)?)?;
    Ok(())
}

pub fn info(dir: &Path) -> Result<Value> {
    let manifest: Value = fs::read(dir.join("manifest.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    Ok(json!({
        "name": dir.file_name().map(|n| n.to_string_lossy().to_string()),
        "path": dir.to_string_lossy(),
        "createdAt": manifest.get("backupCreatedAt"),
        "schemaVersion": manifest.get("schemaVersion"),
        "sizeBytes": filesystem::dir_size(dir),
    }))
}

pub fn list(root: &Path) -> Result<Vec<Value>> {
    let dir = backups_dir(root);
    let mut out = vec![];
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && !e.file_name().to_string_lossy().starts_with('.') && p.join("manifest.json").exists() {
                out.push(info(&p)?);
            }
        }
    }
    out.sort_by(|a, b| b["name"].as_str().cmp(&a["name"].as_str()));
    Ok(out)
}

/// Keeps the newest `keep` backups.
pub fn prune(root: &Path, keep: usize) -> Result<usize> {
    let all = list(root)?;
    let mut removed = 0;
    for b in all.iter().skip(keep.max(1)) {
        if let Some(p) = b["path"].as_str() {
            fs::remove_dir_all(p)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Exports a self-contained zip (database + assets + manifest) for storing on another disk.
pub fn export_zip(conn: &Connection, root: &Path, manifest: &Value, dest_zip: &Path) -> Result<Value> {
    let tmp = backups_dir(root).join(format!(".export-{}", crate::domain::new_id()));
    let result = (|| -> Result<()> {
        write_snapshot(conn, root, manifest, &tmp)?;
        let partial = dest_zip.with_extension("zip.partial");
        let status = Command::new("/usr/bin/ditto").args(["-c", "-k", "--sequesterRsrc"]).arg(&tmp).arg(&partial).status()?;
        if !status.success() {
            let _ = fs::remove_file(&partial);
            return invalid("creating the zip archive failed");
        }
        fs::rename(&partial, dest_zip)?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&tmp);
    result?;
    Ok(json!({ "path": dest_zip.to_string_lossy(), "sizeBytes": fs::metadata(dest_zip)?.len() }))
}

fn find_snapshot_root(dir: &Path, depth: usize) -> Option<PathBuf> {
    if dir.join("workspace.sqlite").exists() && dir.join("manifest.json").exists() {
        return Some(dir.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find_snapshot_root(&e.path(), depth - 1))
}

/// Restores a backup folder or exported zip into a NEW workspace folder. The current workspace is untouched,
/// so the restored copy can be checked before switching to it.
pub fn restore(source: &Path, dest: &Path) -> Result<Value> {
    if dest.exists() && fs::read_dir(dest)?.next().is_some() {
        return invalid("choose an empty or new folder to restore into");
    }
    let mut cleanup = None;
    let src_root = if source.is_file() {
        let tmp = std::env::temp_dir().join(format!("pw-restore-{}", crate::domain::new_id()));
        let status = Command::new("/usr/bin/ditto").args(["-x", "-k"]).arg(source).arg(&tmp).status()?;
        if !status.success() {
            return invalid("could not read the backup archive");
        }
        cleanup = Some(tmp.clone());
        tmp
    } else {
        source.to_path_buf()
    };
    let result = (|| -> Result<Value> {
        let snap = find_snapshot_root(&src_root, 2).ok_or_else(|| AppError::Invalid("no workspace.sqlite and manifest.json found in the backup".into()))?;
        let manifest: Value = serde_json::from_slice(&fs::read(snap.join("manifest.json"))?)?;
        if manifest.get("format").and_then(|f| f.as_str()) != Some(crate::application::workspace::MANIFEST_FORMAT) {
            return invalid("this is not a Personal Workspace backup");
        }
        fs::create_dir_all(dest)?;
        fs::copy(snap.join("workspace.sqlite"), dest.join("workspace.sqlite"))?;
        filesystem::copy_dir_all(&snap.join("assets"), &dest.join("assets"))?;
        fs::copy(snap.join("manifest.json"), dest.join("manifest.json"))?;
        let conn = database::open(&dest.join("workspace.sqlite"))?;
        let report = integrity(&conn, dest)?;
        Ok(json!({ "path": dest.to_string_lossy(), "manifest": manifest, "integrity": report }))
    })();
    if let Some(tmp) = cleanup {
        let _ = fs::remove_dir_all(tmp);
    }
    if result.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    result
}

/// Database integrity, broken references and missing/orphaned attachment files.
pub fn integrity(conn: &Connection, root: &Path) -> Result<Value> {
    let check: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    let fk_problems: i64 = conn.prepare("PRAGMA foreign_key_check")?.query_map([], |_| Ok(()))?.count() as i64;
    let mut referenced: Vec<String> = vec![];
    let mut stmt = conn.prepare("SELECT props FROM blocks WHERE props LIKE '%wsasset://%'")?;
    for raw in stmt.query_map([], |r| r.get::<_, String>(0))?.flatten() {
        let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        if let Some(id) = v.get("url").and_then(|u| u.as_str()).and_then(|u| u.strip_prefix("wsasset://localhost/")) {
            referenced.push(id.to_string());
        }
    }
    let known: Vec<String> = conn.prepare("SELECT id FROM assets")?.query_map([], |r| r.get(0))?.flatten().collect();
    let missing_files: Vec<&String> = known.iter().filter(|id| !root.join("assets").join(id).exists()).collect();
    let missing_refs: Vec<&String> = referenced.iter().filter(|id| !root.join("assets").join(id.as_str()).exists()).collect();
    let orphan_files = fs::read_dir(root.join("assets"))
        .map(|rd| rd.flatten().filter(|e| !known.contains(&e.file_name().to_string_lossy().to_string())).count())
        .unwrap_or(0);
    let broken_links: i64 = conn.query_row(
        "SELECT COUNT(*) FROM links l WHERE NOT EXISTS (SELECT 1 FROM pages p WHERE p.id=l.target_id) AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.id=l.target_id)",
        [],
        |r| r.get(0),
    )?;
    let pages: i64 = conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0))?;
    Ok(json!({
        "ok": check == "ok" && fk_problems == 0 && missing_files.is_empty() && missing_refs.is_empty(),
        "sqlite": check,
        "foreignKeyProblems": fk_problems,
        "missingAssetFiles": missing_files,
        "missingReferencedAssets": missing_refs,
        "orphanAssetFiles": orphan_files,
        "brokenLinks": broken_links,
        "pages": pages,
    }))
}

// ---------- encrypted exports ----------

const MAGIC: &[u8] = b"PWENC1";
const PBKDF2_ROUNDS: u32 = 600_000;

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>> {
    use aes_gcm::aead::{rand_core::RngCore, Aead, AeadCore, KeyInit, OsRng};
    if password.chars().count() < 8 {
        return invalid("use a password of at least 8 characters");
    }
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(&derive_key(password, &salt)).map_err(|_| AppError::Invalid("key error".into()))?;
    let nonce = aes_gcm::Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher.encrypt(&nonce, data).map_err(|_| AppError::Invalid("encryption failed".into()))?;
    Ok([MAGIC, &salt, nonce.as_slice(), &ct].concat())
}

pub fn decrypt(data: &[u8], password: &str) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    if !is_encrypted(data) || data.len() < MAGIC.len() + 16 + 12 + 16 {
        return invalid("this is not an encrypted Personal Workspace backup");
    }
    let (salt, rest) = data[MAGIC.len()..].split_at(16);
    let (nonce, ct) = rest.split_at(12);
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(&derive_key(password, salt)).map_err(|_| AppError::Invalid("key error".into()))?;
    cipher.decrypt(aes_gcm::Nonce::from_slice(nonce), ct).map_err(|_| AppError::Invalid("wrong password, or the file is damaged".into()))
}

pub fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

/// Zip export encrypted with a password (AES-256-GCM, PBKDF2-SHA256 key). There is no recovery without the password.
pub fn export_encrypted(conn: &Connection, root: &Path, manifest: &Value, dest: &Path, password: &str) -> Result<Value> {
    let tmp_zip = std::env::temp_dir().join(format!("pw-export-{}.zip", crate::domain::new_id()));
    let result = (|| -> Result<Value> {
        export_zip(conn, root, manifest, &tmp_zip)?;
        let enc = encrypt(&fs::read(&tmp_zip)?, password)?;
        filesystem::atomic_write(dest, &enc)?;
        Ok(json!({ "path": dest.to_string_lossy(), "sizeBytes": enc.len(), "encrypted": true }))
    })();
    let _ = fs::remove_file(&tmp_zip);
    result
}

/// Restores an encrypted export into a new folder (decrypts to a temporary zip first).
pub fn restore_encrypted(source: &Path, dest: &Path, password: &str) -> Result<Value> {
    let plain = decrypt(&fs::read(source)?, password)?;
    let tmp_zip = std::env::temp_dir().join(format!("pw-restore-{}.zip", crate::domain::new_id()));
    fs::write(&tmp_zip, plain)?;
    let r = restore(&tmp_zip, dest);
    let _ = fs::remove_file(&tmp_zip);
    r
}

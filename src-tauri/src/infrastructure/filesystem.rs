//! File helpers: atomic writes, safe asset names and directory copies.

use crate::error::{invalid, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Writes to a temporary sibling then renames, so a failed export never leaves a file that looks complete.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.partial-{}", crate::domain::new_id()));
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    Ok(result?)
}

/// Asset ids are file names we generated: no separators, no traversal.
pub fn check_asset_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && !id.contains("..")
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok { Ok(()) } else { invalid("invalid asset id") }
}

pub fn asset_path(root: &Path, id: &str) -> Result<PathBuf> {
    check_asset_id(id)?;
    Ok(root.join("assets").join(id))
}

pub fn safe_extension(file_name: &str) -> String {
    Path::new(file_name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| e.len() <= 10 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "bin".into())
}

pub fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "heic" => "image/heic",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "txt" | "md" => "text/plain; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<u64> {
    let mut total = 0;
    fs::create_dir_all(dst)?;
    if !src.exists() {
        return Ok(0);
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            total += copy_dir_all(&entry.path(), &target)?;
        } else {
            total += fs::copy(entry.path(), target)?;
        }
    }
    Ok(total)
}

pub fn dir_size(path: &Path) -> u64 {
    let Ok(rd) = fs::read_dir(path) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            _ => e.metadata().map(|m| m.len()).unwrap_or(0),
        })
        .sum()
}

/// File names safe on APFS while keeping Thai and other Unicode characters.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | ':' | '\\' | '\0') || c.is_control() { '-' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() { "Untitled".into() } else { crate::domain::truncate_chars(trimmed, 120).replace('…', "") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        assert!(check_asset_id("../workspace.sqlite").is_err());
        assert!(check_asset_id("a/b.png").is_err());
        assert!(check_asset_id("0b6f.png").is_ok());
    }

    #[test]
    fn keeps_thai_names() {
        assert_eq!(sanitize_file_name("บันทึก/ประชุม"), "บันทึก-ประชุม");
    }
}

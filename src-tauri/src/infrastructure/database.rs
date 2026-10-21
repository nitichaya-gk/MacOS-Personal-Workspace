//! SQLite connection setup and versioned migrations (tracked with PRAGMA user_version).

use crate::error::{AppError, Result};
use rusqlite::Connection;
use std::path::Path;
use std::time::Duration;

const MIGRATIONS: &[&str] = &[include_str!("../../migrations/0001_init.sql"), include_str!("../../migrations/0002_growth.sql"), include_str!("../../migrations/0003_phase3.sql"), include_str!("../../migrations/0004_timetable.sql"), include_str!("../../migrations/0005_growth2.sql")];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

pub fn open(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    configure(&conn)?;
    migrate(&conn, db_path)?;
    Ok(conn)
}

pub fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))?;
    // FULL: anything reported as saved survives a crash or power loss.
    conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")?;
    Ok(())
}

pub fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

fn migrate(conn: &Connection, db_path: &Path) -> Result<()> {
    let current = schema_version(conn)?;
    if current > SCHEMA_VERSION {
        return Err(AppError::Invalid(format!(
            "This workspace uses schema v{current}, newer than this app (v{SCHEMA_VERSION}). Update the app to open it."
        )));
    }
    if current == SCHEMA_VERSION {
        return Ok(());
    }
    if current > 0 {
        // Keep an untouched copy so a failed migration never destroys the original data.
        if let Some(dir) = db_path.parent() {
            let backups = dir.join("backups");
            std::fs::create_dir_all(&backups)?;
            let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
            let dest = backups.join(format!("pre-migration-v{current}-{stamp}.sqlite"));
            conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])?;
        }
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = crate::infrastructure::database::begin(conn)?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// A transaction that nests: the outermost call issues BEGIN/COMMIT, inner calls use SAVEPOINTs, so
/// services can call each other and a failing inner step rolls back only its own changes.
pub struct Tx<'a> {
    conn: &'a Connection,
    savepoint: Option<String>,
    done: bool,
}

static SAVEPOINT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn begin(conn: &Connection) -> Result<Tx<'_>> {
    if conn.is_autocommit() {
        conn.execute_batch("BEGIN")?;
        Ok(Tx { conn, savepoint: None, done: false })
    } else {
        let name = format!("sp{}", SAVEPOINT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        conn.execute_batch(&format!("SAVEPOINT {name}"))?;
        Ok(Tx { conn, savepoint: Some(name), done: false })
    }
}

impl Tx<'_> {
    pub fn commit(mut self) -> Result<()> {
        match &self.savepoint {
            None => self.conn.execute_batch("COMMIT")?,
            Some(n) => self.conn.execute_batch(&format!("RELEASE {n}"))?,
        }
        self.done = true;
        Ok(())
    }
}

impl std::ops::Deref for Tx<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.conn
    }
}

impl Drop for Tx<'_> {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let _ = match &self.savepoint {
            None => self.conn.execute_batch("ROLLBACK"),
            Some(n) => self.conn.execute_batch(&format!("ROLLBACK TO {n}; RELEASE {n}")),
        };
    }
}

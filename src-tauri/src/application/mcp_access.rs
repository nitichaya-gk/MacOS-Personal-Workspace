//! Which workspace data MCP clients may read. Disabled until the user turns it on;
//! scope is "all" or a list of allowed section/page ids, and a database can always opt out.

use super::{pages, settings};
use crate::error::{AppError, Result};
use rusqlite::{params, Connection};
use serde_json::Value;

pub struct Policy {
    pub enabled: bool,
    pub all: bool,
    pub allowed: Vec<String>,
}

impl Policy {
    pub fn load(conn: &Connection) -> Result<Self> {
        let v = settings::get(conn, "mcp")?;
        Ok(Policy {
            enabled: v.get("enabled").and_then(|e| e.as_bool()).unwrap_or(false),
            all: v.get("scope").and_then(|s| s.as_str()) == Some("all"),
            allowed: v
                .get("allowedIds")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        })
    }

    pub fn can_read_page(&self, conn: &Connection, page_id: &str) -> Result<bool> {
        if !self.enabled {
            return Ok(false);
        }
        let Ok(me) = pages::get_meta(conn, page_id) else { return Ok(false) };
        if me.deleted_at.is_some() {
            return Ok(false);
        }
        let mut chain = pages::ancestors(conn, page_id)?;
        chain.push(me.clone());
        for p in &chain {
            if p.kind == "database" {
                let access: String = conn.query_row("SELECT mcp_access FROM databases WHERE id=?1", [&p.id], |r| r.get(0))?;
                if access == "deny" {
                    return Ok(false);
                }
            }
        }
        if self.all {
            return Ok(true);
        }
        Ok(chain.iter().any(|p| self.allowed.contains(&p.id))
            || me.section_id.as_ref().is_some_and(|s| self.allowed.contains(s)))
    }

    pub fn can_read_task(&self, conn: &Connection, project_id: Option<&str>) -> Result<bool> {
        if !self.enabled {
            return Ok(false);
        }
        if self.all {
            return Ok(true);
        }
        match project_id {
            Some(p) => self.can_read_page(conn, p),
            None => Ok(self.allowed.iter().any(|a| a == "tasks:unassigned")),
        }
    }

    /// Growth domains: goals, skills, habits, finance. Finance (sensitive) needs an explicit grant even with scope "all".
    pub fn domain_allowed(&self, domain: &str) -> bool {
        self.enabled && (self.allowed.iter().any(|a| a == &format!("domain:{domain}")) || (self.all && domain != "finance"))
    }

    pub fn can_read_goal(&self, goal_id: &str) -> bool {
        self.enabled && (self.domain_allowed("goals") || self.allowed.iter().any(|a| a == goal_id))
    }

    pub fn require_goal(&self, goal_id: &str) -> Result<()> {
        if self.can_read_goal(goal_id) {
            Ok(())
        } else {
            Err(AppError::Denied(format!("goal {goal_id} is not shared with MCP clients")))
        }
    }

    pub fn require_domain(&self, domain: &str) -> Result<()> {
        if self.domain_allowed(domain) {
            Ok(())
        } else {
            Err(AppError::Denied(format!("{domain} data is not shared with MCP clients (Settings → MCP)")))
        }
    }

    pub fn require_page(&self, conn: &Connection, page_id: &str) -> Result<()> {
        if self.can_read_page(conn, page_id)? {
            Ok(())
        } else {
            Err(AppError::Denied(format!("{page_id} is outside the scope shared with MCP clients")))
        }
    }
}

pub fn audit(conn: &Connection, client: &str, tool: &str, target: Option<&str>, result: &str, detail: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT INTO mcp_audit_logs(id, client, tool, target, result, detail, at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![crate::domain::new_id(), client, tool, target, result, detail, crate::domain::now()],
    )?;
    // Keep the log bounded.
    conn.execute("DELETE FROM mcp_audit_logs WHERE at < ?1", [(chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339()])?;
    Ok(())
}

pub fn audit_log(conn: &Connection, limit: usize) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare("SELECT client, tool, target, result, detail, at FROM mcp_audit_logs ORDER BY at DESC LIMIT ?1")?;
    let rows = stmt.query_map([limit as i64], |r| {
        Ok(serde_json::json!({
            "client": r.get::<_, String>(0)?, "tool": r.get::<_, String>(1)?, "target": r.get::<_, Option<String>>(2)?,
            "result": r.get::<_, String>(3)?, "detail": r.get::<_, Option<String>>(4)?, "at": r.get::<_, String>(5)?
        }))
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

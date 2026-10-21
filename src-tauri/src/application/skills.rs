//! Skills with per-skill rubrics, evidence (the only thing that moves a skill level) and focus sessions.

use super::tasks;
use crate::domain::growth::{self, EvidenceInput};
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{json, Value};

pub const EVIDENCE_TYPES: &[&str] = &["assessment", "mock_test", "artifact", "code_review", "project", "certificate", "self_review"];

fn default_rubric() -> Value {
    json!([
        "Not assessed",
        "Recognize: knows the terms and can follow an example",
        "Apply with guidance",
        "Apply independently",
        "Solve complex cases",
        "Teach or design new solutions"
    ])
}

fn skill_row(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "category": r.get::<_, String>(2)?, "description": r.get::<_, String>(3)?,
        "targetLevel": r.get::<_, i64>(4)?, "rubric": serde_json::from_str::<Value>(&r.get::<_, String>(5)?).unwrap_or(json!([])),
        "rubricVersion": r.get::<_, i64>(6)?, "createdAt": r.get::<_, String>(7)?, "updatedAt": r.get::<_, String>(8)?,
    }))
}
const SKILL_COLS: &str = "id, name, category, description, target_level, rubric_json, rubric_version, created_at, updated_at";

pub fn evidence_inputs(conn: &Connection, skill_id: &str) -> Result<Vec<EvidenceInput>> {
    let rows = conn
        .prepare("SELECT level, observed_at, evidence_type FROM skill_evidence WHERE skill_id=?1")?
        .query_map([skill_id], |r| Ok(EvidenceInput { level: r.get(0)?, observed_at: r.get(1)?, etype: r.get(2)? }))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: &str) -> Result<Value> {
    let mut s = conn
        .query_row(&format!("SELECT {SKILL_COLS} FROM skills WHERE id=?1 AND deleted_at IS NULL"), [id], skill_row)
        .optional()?
        .ok_or_else(|| AppError::NotFound("skill".into()))?;
    let today = growth::date(&tasks::today(conn)).unwrap();
    s["readiness"] = serde_json::to_value(growth::skill_readiness(&evidence_inputs(conn, id)?, today))?;
    s["practiceTasks"] = json!(conn.query_row(
        "SELECT COUNT(*) FROM task_skill_links l JOIN tasks t ON t.id=l.task_id WHERE l.skill_id=?1 AND t.deleted_at IS NULL AND t.status='done'",
        [id], |r| r.get::<_, i64>(0))?);
    s["goals"] = json!(conn
        .prepare("SELECT g.id, g.title, gs.target_level FROM goal_skills gs JOIN goals g ON g.id=gs.goal_id WHERE gs.skill_id=?1 AND g.deleted_at IS NULL")?
        .query_map([id], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "targetLevel": r.get::<_, i64>(2)? })))?
        .collect::<std::result::Result<Vec<_>, _>>()?);
    Ok(s)
}

pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM skills WHERE deleted_at IS NULL ORDER BY category, name")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    ids.iter().map(|id| get(conn, id)).collect()
}

pub fn create(conn: &Connection, p: &Value) -> Result<Value> {
    let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("").trim();
    if name.is_empty() {
        return invalid("skill name is required");
    }
    let id = new_id();
    conn.execute(
        "INSERT INTO skills(id, name, rubric_json, created_at, updated_at) VALUES (?1,?2,?3,?4,?4)",
        params![id, name, default_rubric().to_string(), now()],
    )?;
    let mut rest = p.clone();
    rest.as_object_mut().map(|o| o.remove("name"));
    update(conn, &id, &rest)
}

pub fn update(conn: &Connection, id: &str, p: &Value) -> Result<Value> {
    get(conn, id)?;
    for (col, key) in [("name", "name"), ("category", "category"), ("description", "description")] {
        if let Some(v) = p.get(key).and_then(|v| v.as_str()) {
            if col == "name" && v.trim().is_empty() {
                return invalid("skill name is required");
            }
            conn.execute(&format!("UPDATE skills SET {col}=?2 WHERE id=?1"), params![id, v.trim()])?;
        }
    }
    if let Some(l) = p.get("targetLevel").and_then(|v| v.as_i64()) {
        conn.execute("UPDATE skills SET target_level=?2 WHERE id=?1", params![id, l.clamp(0, 5)])?;
    }
    if let Some(r) = p.get("rubric").and_then(|v| v.as_array()) {
        if r.len() != 6 || !r.iter().all(|x| x.is_string()) {
            return invalid("a rubric describes levels 0 to 5 (6 entries)");
        }
        // Changing the rubric bumps its version so older evidence stays tied to the rubric it was scored with.
        conn.execute("UPDATE skills SET rubric_json=?2, rubric_version=rubric_version+1 WHERE id=?1", params![id, Value::Array(r.clone()).to_string()])?;
    }
    conn.execute("UPDATE skills SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE skills SET deleted_at=?2 WHERE id=?1", params![id, now()])?;
    Ok(())
}

fn evidence_row(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "skillId": r.get::<_, String>(1)?, "evidenceType": r.get::<_, String>(2)?, "title": r.get::<_, String>(3)?,
        "level": r.get::<_, Option<i64>>(4)?, "score": r.get::<_, Option<f64>>(5)?, "maxScore": r.get::<_, Option<f64>>(6)?,
        "rubricVersion": r.get::<_, Option<i64>>(7)?, "taskId": r.get::<_, Option<String>>(8)?, "requirementId": r.get::<_, Option<String>>(9)?,
        "sourceItemId": r.get::<_, Option<String>>(10)?, "sourceUrl": r.get::<_, String>(11)?, "observedAt": r.get::<_, String>(12)?,
        "reviewer": r.get::<_, String>(13)?, "notes": r.get::<_, String>(14)?, "createdAt": r.get::<_, String>(15)?,
        "skillName": r.get::<_, Option<String>>(16)?, "taskTitle": r.get::<_, Option<String>>(17)?, "sourceTitle": r.get::<_, Option<String>>(18)?,
    }))
}
const EV_SELECT: &str = "SELECT e.id, e.skill_id, e.evidence_type, e.title, e.level, e.score, e.max_score, e.rubric_version, e.task_id, e.requirement_id,
    e.source_item_id, e.source_url, e.observed_at, e.reviewer, e.notes, e.created_at, s.name, t.title, p.title
    FROM skill_evidence e LEFT JOIN skills s ON s.id=e.skill_id LEFT JOIN tasks t ON t.id=e.task_id LEFT JOIN pages p ON p.id=e.source_item_id";

pub fn evidence_for_skill(conn: &Connection, skill_id: &str) -> Result<Vec<Value>> {
    let rows = conn.prepare(&format!("{EV_SELECT} WHERE e.skill_id=?1 ORDER BY e.observed_at DESC, e.created_at DESC"))?.query_map([skill_id], evidence_row)?.collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

pub fn evidence_for_requirement(conn: &Connection, req_id: &str) -> Result<Vec<Value>> {
    let rows = conn.prepare(&format!("{EV_SELECT} WHERE e.requirement_id=?1 ORDER BY e.observed_at DESC"))?.query_map([req_id], evidence_row)?.collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

pub fn evidence_between(conn: &Connection, from: &str, to: &str) -> Result<Vec<Value>> {
    let rows = conn.prepare(&format!("{EV_SELECT} WHERE e.observed_at >= ?1 AND e.observed_at <= ?2 ORDER BY e.observed_at"))?.query_map([from, to], evidence_row)?.collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

/// Adds evidence with provenance. A score can optionally become the requirement's measured value.
pub fn add_evidence(conn: &Connection, p: &Value) -> Result<Value> {
    let skill_id = p.get("skillId").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("skillId is required".into()))?;
    let skill = get(conn, skill_id)?;
    let etype = p.get("evidenceType").and_then(|v| v.as_str()).unwrap_or("artifact");
    if !EVIDENCE_TYPES.contains(&etype) {
        return invalid("unknown evidence type");
    }
    let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("").trim();
    if title.is_empty() {
        return invalid("describe the evidence");
    }
    let level = p.get("level").and_then(|v| v.as_i64());
    if level.is_some_and(|l| !(0..=5).contains(&l)) {
        return invalid("rubric level must be 0–5");
    }
    let score = p.get("score").and_then(|v| v.as_f64());
    let max = p.get("maxScore").and_then(|v| v.as_f64());
    if let (Some(s), Some(m)) = (score, max) {
        if m <= 0.0 || s < 0.0 || s > m {
            return invalid("score must be between 0 and the maximum");
        }
    }
    let observed = p.get("observedAt").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| tasks::today(conn));
    if growth::date(&observed).is_none() {
        return invalid("date must be YYYY-MM-DD");
    }
    let id = new_id();
    let s = |k: &str| p.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(String::from);
    conn.execute(
        "INSERT INTO skill_evidence(id, skill_id, evidence_type, title, level, score, max_score, rubric_version, task_id, requirement_id, source_item_id, source_url, observed_at, reviewer, notes, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            id, skill_id, etype, title, level, score, max, level.map(|_| skill["rubricVersion"].as_i64().unwrap_or(1)), s("taskId"), s("requirementId"),
            s("sourceItemId"), s("sourceUrl").unwrap_or_default(), observed, s("reviewer").unwrap_or_default(), s("notes").unwrap_or_default(), now()
        ],
    )?;
    if p.get("updateRequirementValue").and_then(|v| v.as_bool()) == Some(true) {
        if let (Some(rid), Some(sc)) = (s("requirementId"), score) {
            super::goals::update_requirement(conn, &rid, &json!({ "currentValue": sc, "satisfactionStatus": "in_progress" }))?;
        }
    }
    Ok(conn.query_row(&format!("{EV_SELECT} WHERE e.id=?1"), [&id], evidence_row)?)
}

pub fn delete_evidence(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM skill_evidence WHERE id=?1", [id])?;
    Ok(())
}

// ---------- focus sessions ----------

fn session_row(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "taskId": r.get::<_, Option<String>>(1)?, "goalId": r.get::<_, Option<String>>(2)?, "skillId": r.get::<_, Option<String>>(3)?,
        "startedAt": r.get::<_, String>(4)?, "endedAt": r.get::<_, Option<String>>(5)?, "plannedMinutes": r.get::<_, Option<i64>>(6)?,
        "actualMinutes": r.get::<_, Option<i64>>(7)?, "focusQuality": r.get::<_, Option<i64>>(8)?, "energy": r.get::<_, Option<i64>>(9)?,
        "note": r.get::<_, String>(10)?, "taskTitle": r.get::<_, Option<String>>(11)?, "goalTitle": r.get::<_, Option<String>>(12)?,
    }))
}
const FS_SELECT: &str = "SELECT f.id, f.task_id, f.goal_id, f.skill_id, f.started_at, f.ended_at, f.planned_minutes, f.actual_minutes, f.focus_quality, f.energy, f.note, t.title, g.title
    FROM focus_sessions f LEFT JOIN tasks t ON t.id=f.task_id LEFT JOIN goals g ON g.id=f.goal_id";

pub fn running_session(conn: &Connection) -> Result<Option<Value>> {
    Ok(conn.query_row(&format!("{FS_SELECT} WHERE f.ended_at IS NULL ORDER BY f.started_at DESC LIMIT 1"), [], session_row).optional()?)
}

/// Starts a session (persisted, so a running timer survives restarts). Only one can run at a time.
pub fn start_session(conn: &Connection, p: &Value) -> Result<Value> {
    if running_session(conn)?.is_some() {
        return invalid("a focus session is already running");
    }
    let id = new_id();
    let s = |k: &str| p.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(String::from);
    let mut goal = s("goalId");
    if goal.is_none() {
        if let Some(t) = s("taskId") {
            // Infer the goal from the task's milestone so time shows up under the goal.
            goal = conn.query_row("SELECT m.goal_id FROM tasks t JOIN milestones m ON m.id=t.milestone_id WHERE t.id=?1", [&t], |r| r.get(0)).optional()?;
        }
    }
    conn.execute(
        "INSERT INTO focus_sessions(id, task_id, goal_id, skill_id, started_at, planned_minutes, created_at) VALUES (?1,?2,?3,?4,?5,?6,?5)",
        params![id, s("taskId"), goal, s("skillId"), now(), p.get("plannedMinutes").and_then(|v| v.as_i64())],
    )?;
    Ok(conn.query_row(&format!("{FS_SELECT} WHERE f.id=?1"), [&id], session_row)?)
}

pub fn stop_session(conn: &Connection, id: &str, p: &Value) -> Result<Value> {
    let started: String = conn.query_row("SELECT started_at FROM focus_sessions WHERE id=?1", [id], |r| r.get(0))?;
    let start = chrono::DateTime::parse_from_rfc3339(&started).map_err(|_| AppError::Invalid("bad start time".into()))?;
    let minutes = p.get("actualMinutes").and_then(|v| v.as_i64()).unwrap_or_else(|| (chrono::Utc::now() - start.with_timezone(&chrono::Utc)).num_minutes().max(0));
    let rating = |k: &str| p.get(k).and_then(|v| v.as_i64()).map(|v| v.clamp(1, 5));
    conn.execute(
        "UPDATE focus_sessions SET ended_at=?2, actual_minutes=?3, focus_quality=?4, energy=?5, note=?6 WHERE id=?1",
        params![id, now(), minutes, rating("focusQuality"), rating("energy"), p.get("note").and_then(|v| v.as_str()).unwrap_or("")],
    )?;
    // Roll session time into the task's actual effort.
    conn.execute(
        "UPDATE tasks SET actual_effort_minutes = COALESCE(actual_effort_minutes,0) + ?2 WHERE id = (SELECT task_id FROM focus_sessions WHERE id=?1)",
        params![id, minutes],
    )?;
    Ok(conn.query_row(&format!("{FS_SELECT} WHERE f.id=?1"), [id], session_row)?)
}

pub fn discard_session(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM focus_sessions WHERE id=?1 AND ended_at IS NULL", [id])?;
    Ok(())
}

pub fn sessions_between(conn: &Connection, from_utc: &str, to_utc: &str) -> Result<Vec<Value>> {
    let rows = conn
        .prepare(&format!("{FS_SELECT} WHERE f.ended_at IS NOT NULL AND f.started_at >= ?1 AND f.started_at < ?2 ORDER BY f.started_at DESC"))?
        .query_map([from_utc, to_utc], session_row)?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

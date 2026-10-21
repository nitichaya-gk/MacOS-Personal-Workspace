//! Goals, requirements, milestones, goal metrics/health and AI drafts.

use super::{skills, tasks};
use crate::domain::growth::{self, EvidenceInput, HealthInput, MilestoneInput, ReqInput, TaskInput};
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use crate::infrastructure::search;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{json, Map, Value};

pub const DOMAINS: &[&str] = &["education", "career", "finance", "personal", "project"];
const DEFAULT_EFFORT_MIN: f64 = 60.0;

fn today(conn: &Connection) -> chrono::NaiveDate {
    growth::date(&tasks::today(conn)).unwrap_or_else(|| chrono::Utc::now().date_naive())
}

fn opt_str(v: &Value, k: &str) -> Option<Option<String>> {
    v.get(k).map(|x| x.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

const GOAL_COLS: &str = "id, title, description, domain, motivation, start_date, target_date, status, priority, time_budget_hours_per_week,
    budget_limit_minor, currency, success_definition, risk_level, confidence, notes, project_id, last_reviewed_at, is_example, created_at, updated_at, deleted_at, version";

fn goal_row(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "description": r.get::<_, String>(2)?,
        "domain": r.get::<_, String>(3)?, "motivation": r.get::<_, String>(4)?, "startDate": r.get::<_, Option<String>>(5)?,
        "targetDate": r.get::<_, Option<String>>(6)?, "status": r.get::<_, String>(7)?, "priority": r.get::<_, i64>(8)?,
        "timeBudgetHoursPerWeek": r.get::<_, Option<f64>>(9)?, "budgetLimitMinor": r.get::<_, Option<i64>>(10)?,
        "currency": r.get::<_, String>(11)?, "successDefinition": r.get::<_, String>(12)?, "riskLevel": r.get::<_, String>(13)?,
        "confidence": r.get::<_, String>(14)?, "notes": r.get::<_, String>(15)?, "projectId": r.get::<_, Option<String>>(16)?,
        "lastReviewedAt": r.get::<_, Option<String>>(17)?, "isExample": r.get::<_, i64>(18)? != 0,
        "createdAt": r.get::<_, String>(19)?, "updatedAt": r.get::<_, String>(20)?, "deletedAt": r.get::<_, Option<String>>(21)?,
        "version": r.get::<_, i64>(22)?,
    }))
}

pub fn get_goal(conn: &Connection, id: &str) -> Result<Value> {
    conn.query_row(&format!("SELECT {GOAL_COLS} FROM goals WHERE id=?1"), [id], goal_row)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("goal {id}")))
}

// ---------- requirements & milestones ----------

pub fn requirements(conn: &Connection, goal_id: &str) -> Result<Vec<ReqInput>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, type, critical, weight, target_value, current_value, satisfaction_status, verification_status, verification_source, source_checked_at, deadline
         FROM goal_requirements WHERE goal_id=?1 ORDER BY position",
    )?;
    let rows = stmt.query_map([goal_id], |r| {
        Ok(ReqInput {
            id: r.get(0)?, title: r.get(1)?, rtype: r.get(2)?, critical: r.get::<_, i64>(3)? != 0, weight: r.get(4)?,
            target: r.get(5)?, current: r.get(6)?, satisfaction: r.get(7)?, verification: r.get(8)?, verification_source: r.get(9)?,
            source_checked_at: r.get(10)?, deadline: r.get(11)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn requirement_json(conn: &Connection, id: &str) -> Result<Value> {
    conn.query_row(
        "SELECT id, goal_id, title, description, type, critical, weight, target_value, current_value, unit, deadline, verification_source,
                source_checked_at, verification_status, satisfaction_status, manual_override_reason, position
         FROM goal_requirements WHERE id=?1",
        [id],
        |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?, "goalId": r.get::<_, String>(1)?, "title": r.get::<_, String>(2)?, "description": r.get::<_, String>(3)?,
                "type": r.get::<_, String>(4)?, "critical": r.get::<_, i64>(5)? != 0, "weight": r.get::<_, f64>(6)?,
                "targetValue": r.get::<_, Option<f64>>(7)?, "currentValue": r.get::<_, Option<f64>>(8)?, "unit": r.get::<_, String>(9)?,
                "deadline": r.get::<_, Option<String>>(10)?, "verificationSource": r.get::<_, String>(11)?, "sourceCheckedAt": r.get::<_, Option<String>>(12)?,
                "verificationStatus": r.get::<_, String>(13)?, "satisfactionStatus": r.get::<_, String>(14)?, "manualOverrideReason": r.get::<_, String>(15)?,
                "position": r.get::<_, f64>(16)?,
            }))
        },
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("requirement".into()))
}

const REQ_TYPES: &[&str] = &["binary", "numeric", "score", "document", "portfolio", "external_verification"];
const SATISFACTION: &[&str] = &["not_started", "in_progress", "met", "not_met", "blocked", "unknown"];

pub fn add_requirement(conn: &Connection, goal_id: &str, p: &Value) -> Result<Value> {
    get_goal(conn, goal_id)?;
    let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
    if title.is_empty() {
        return invalid("requirement title is required");
    }
    let id = new_id();
    let pos: f64 = conn.query_row("SELECT COALESCE(MAX(position)+1,0) FROM goal_requirements WHERE goal_id=?1", [goal_id], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO goal_requirements(id, goal_id, title, position, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
        params![id, goal_id, title, pos, now()],
    )?;
    let mut rest = p.clone();
    rest.as_object_mut().map(|o| o.remove("title"));
    update_requirement(conn, &id, &rest)
}

pub fn update_requirement(conn: &Connection, id: &str, p: &Value) -> Result<Value> {
    let before = requirement_json(conn, id)?;
    let tx = crate::infrastructure::database::begin(conn)?;
    let text = |col: &str, key: &str| -> Result<()> {
        if let Some(v) = opt_str(p, key) {
            tx.execute(&format!("UPDATE goal_requirements SET {col}=?2 WHERE id=?1"), params![id, v.unwrap_or_default()])?;
        }
        Ok(())
    };
    let nullable = |col: &str, key: &str| -> Result<()> {
        if let Some(v) = opt_str(p, key) {
            tx.execute(&format!("UPDATE goal_requirements SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
        Ok(())
    };
    if let Some(t) = p.get("title").and_then(|t| t.as_str()) {
        if t.trim().is_empty() {
            return invalid("requirement title is required");
        }
    }
    text("title", "title")?;
    text("description", "description")?;
    text("unit", "unit")?;
    text("verification_source", "verificationSource")?;
    text("manual_override_reason", "manualOverrideReason")?;
    nullable("deadline", "deadline")?;
    nullable("source_checked_at", "sourceCheckedAt")?;
    if let Some(t) = p.get("type").and_then(|t| t.as_str()) {
        if !REQ_TYPES.contains(&t) {
            return invalid("unknown requirement type");
        }
        tx.execute("UPDATE goal_requirements SET type=?2 WHERE id=?1", params![id, t])?;
    }
    if let Some(c) = p.get("critical").and_then(|t| t.as_bool()) {
        tx.execute("UPDATE goal_requirements SET critical=?2 WHERE id=?1", params![id, c as i64])?;
    }
    if let Some(w) = p.get("weight") {
        let w = w.as_f64().ok_or_else(|| AppError::Invalid("weight must be a number".into()))?;
        if w < 0.0 {
            return invalid("weight cannot be negative");
        }
        tx.execute("UPDATE goal_requirements SET weight=?2 WHERE id=?1", params![id, w])?;
    }
    for (col, key) in [("target_value", "targetValue"), ("current_value", "currentValue")] {
        if let Some(v) = p.get(key) {
            tx.execute(&format!("UPDATE goal_requirements SET {col}=?2 WHERE id=?1"), params![id, v.as_f64()])?;
        }
    }
    if let Some(v) = p.get("verificationStatus").and_then(|t| t.as_str()) {
        if !matches!(v, "unverified" | "verified" | "outdated") {
            return invalid("unknown verification status");
        }
        tx.execute("UPDATE goal_requirements SET verification_status=?2 WHERE id=?1", params![id, v])?;
        if v == "verified" && p.get("sourceCheckedAt").is_none() {
            tx.execute("UPDATE goal_requirements SET source_checked_at=?2 WHERE id=?1", params![id, tasks::today(&tx)])?;
        }
    }
    if let Some(v) = p.get("satisfactionStatus").and_then(|t| t.as_str()) {
        if !SATISFACTION.contains(&v) {
            return invalid("unknown status");
        }
        tx.execute("UPDATE goal_requirements SET satisfaction_status=?2 WHERE id=?1", params![id, v])?;
    }
    // Marking a measured requirement "met" below its target needs an explicit reason.
    let after: (String, String, Option<f64>, Option<f64>, String) = tx.query_row(
        "SELECT satisfaction_status, type, target_value, current_value, manual_override_reason FROM goal_requirements WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    if after.0 == "met" && matches!(after.1.as_str(), "numeric" | "score") {
        if let (Some(t), c) = (after.2, after.3) {
            if c.unwrap_or(0.0) < t && after.4.trim().is_empty() {
                return invalid("the measured value is below the target — add a reason to mark it met anyway");
            }
        }
    }
    tx.execute("UPDATE goal_requirements SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    tx.commit()?;
    let goal_id = before["goalId"].as_str().unwrap_or("").to_string();
    touch(conn, &goal_id)?;
    requirement_json(conn, id)
}

pub fn delete_requirement(conn: &Connection, id: &str) -> Result<()> {
    let r = requirement_json(conn, id)?;
    conn.execute("DELETE FROM goal_requirements WHERE id=?1", [id])?;
    touch(conn, r["goalId"].as_str().unwrap_or(""))
}

fn milestone_json(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "goalId": r.get::<_, String>(1)?, "requirementId": r.get::<_, Option<String>>(2)?,
        "phase": r.get::<_, String>(3)?, "title": r.get::<_, String>(4)?, "dueDate": r.get::<_, Option<String>>(5)?,
        "status": r.get::<_, String>(6)?, "acceptanceCriteria": r.get::<_, String>(7)?, "weight": r.get::<_, f64>(8)?, "position": r.get::<_, f64>(9)?,
    }))
}
const MS_COLS: &str = "id, goal_id, requirement_id, phase, title, due_date, status, acceptance_criteria, weight, position";

pub fn milestones(conn: &Connection, goal_id: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(&format!("SELECT {MS_COLS} FROM milestones WHERE goal_id=?1 ORDER BY position"))?;
    let rows = stmt.query_map([goal_id], milestone_json)?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn get_milestone(conn: &Connection, id: &str) -> Result<Value> {
    conn.query_row(&format!("SELECT {MS_COLS} FROM milestones WHERE id=?1"), [id], milestone_json)
        .optional()?
        .ok_or_else(|| AppError::NotFound("milestone".into()))
}

pub fn add_milestone(conn: &Connection, goal_id: &str, p: &Value) -> Result<Value> {
    get_goal(conn, goal_id)?;
    let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
    if title.is_empty() {
        return invalid("milestone title is required");
    }
    let id = new_id();
    let pos: f64 = conn.query_row("SELECT COALESCE(MAX(position)+1,0) FROM milestones WHERE goal_id=?1", [goal_id], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO milestones(id, goal_id, title, position, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
        params![id, goal_id, title, pos, now()],
    )?;
    let mut rest = p.clone();
    rest.as_object_mut().map(|o| o.remove("title"));
    update_milestone(conn, &id, &rest)
}

pub fn update_milestone(conn: &Connection, id: &str, p: &Value) -> Result<Value> {
    let m = get_milestone(conn, id)?;
    for (col, key) in [("title", "title"), ("phase", "phase"), ("acceptance_criteria", "acceptanceCriteria")] {
        if let Some(v) = opt_str(p, key) {
            if col == "title" && v.is_none() {
                return invalid("milestone title is required");
            }
            conn.execute(&format!("UPDATE milestones SET {col}=?2 WHERE id=?1"), params![id, v.unwrap_or_default()])?;
        }
    }
    for (col, key) in [("due_date", "dueDate"), ("requirement_id", "requirementId")] {
        if let Some(v) = opt_str(p, key) {
            conn.execute(&format!("UPDATE milestones SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    if let Some(s) = p.get("status").and_then(|s| s.as_str()) {
        if !matches!(s, "planned" | "in_progress" | "met" | "missed") {
            return invalid("unknown milestone status");
        }
        conn.execute("UPDATE milestones SET status=?2 WHERE id=?1", params![id, s])?;
    }
    if let Some(w) = p.get("weight").and_then(|w| w.as_f64()) {
        if w < 0.0 {
            return invalid("weight cannot be negative");
        }
        conn.execute("UPDATE milestones SET weight=?2 WHERE id=?1", params![id, w])?;
    }
    if let Some(pos) = p.get("position").and_then(|w| w.as_f64()) {
        conn.execute("UPDATE milestones SET position=?2 WHERE id=?1", params![id, pos])?;
    }
    conn.execute("UPDATE milestones SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    touch(conn, m["goalId"].as_str().unwrap_or(""))?;
    get_milestone(conn, id)
}

pub fn delete_milestone(conn: &Connection, id: &str) -> Result<()> {
    let m = get_milestone(conn, id)?;
    conn.execute("DELETE FROM milestones WHERE id=?1", [id])?;
    touch(conn, m["goalId"].as_str().unwrap_or(""))
}

/// New task inside a milestone; it contributes 100% to the milestone's requirement unless told otherwise.
pub fn add_milestone_task(conn: &Connection, milestone_id: &str, p: &Value) -> Result<tasks::Task> {
    let m = get_milestone(conn, milestone_id)?;
    let goal = get_goal(conn, m["goalId"].as_str().unwrap_or(""))?;
    let mut t = p.clone();
    let o = t.as_object_mut().ok_or_else(|| AppError::Invalid("task must be an object".into()))?;
    o.insert("milestoneId".into(), json!(milestone_id));
    if !o.contains_key("projectId") {
        o.insert("projectId".into(), goal["projectId"].clone());
    }
    if !o.contains_key("requirementLinks") {
        if let Some(rid) = m["requirementId"].as_str() {
            o.insert("requirementLinks".into(), json!([{ "requirementId": rid, "allocationPct": 100 }]));
        }
    }
    tasks::create(conn, &t)
}

// ---------- goal CRUD ----------

fn touch(conn: &Connection, goal_id: &str) -> Result<()> {
    conn.execute("UPDATE goals SET updated_at=?2, version=version+1 WHERE id=?1", params![goal_id, now()])?;
    index(conn, goal_id)
}

pub fn index(conn: &Connection, goal_id: &str) -> Result<()> {
    let Ok(g) = get_goal(conn, goal_id) else { return Ok(()) };
    let reqs: Vec<String> = requirements(conn, goal_id)?.into_iter().map(|r| r.title).collect();
    let body = format!("{}\n{}\n{}", g["description"].as_str().unwrap_or(""), g["successDefinition"].as_str().unwrap_or(""), reqs.join("\n"));
    search::upsert(conn, goal_id, "goal", g["projectId"].as_str(), g["updatedAt"].as_str().unwrap_or(""), g["title"].as_str().unwrap_or(""), &body)
}

pub fn create(conn: &Connection, p: &Value) -> Result<Value> {
    let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
    if title.is_empty() {
        return invalid("goal title is required");
    }
    let domain = p.get("domain").and_then(|t| t.as_str()).unwrap_or("personal");
    if !DOMAINS.contains(&domain) {
        return invalid("unknown goal domain");
    }
    let id = new_id();
    let ts = now();
    conn.execute(
        "INSERT INTO goals(id, title, domain, start_date, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
        params![id, title, domain, tasks::today(conn), ts],
    )?;
    let mut rest = p.clone();
    if let Some(o) = rest.as_object_mut() {
        o.remove("title");
        o.remove("domain");
    }
    update(conn, &id, &rest)
}

pub fn update(conn: &Connection, id: &str, p: &Value) -> Result<Value> {
    let g = get_goal(conn, id)?;
    if let Some(v) = p.get("version").and_then(|v| v.as_i64()) {
        if v != g["version"].as_i64().unwrap_or(0) {
            return invalid("this goal was changed elsewhere — reload and try again");
        }
    }
    let tx = crate::infrastructure::database::begin(conn)?;
    for (col, key) in [("title", "title"), ("description", "description"), ("motivation", "motivation"), ("success_definition", "successDefinition"), ("notes", "notes")] {
        if let Some(v) = opt_str(p, key) {
            if col == "title" && v.is_none() {
                return invalid("goal title is required");
            }
            tx.execute(&format!("UPDATE goals SET {col}=?2 WHERE id=?1"), params![id, v.unwrap_or_default()])?;
        }
    }
    for (col, key) in [("start_date", "startDate"), ("target_date", "targetDate"), ("project_id", "projectId")] {
        if let Some(v) = opt_str(p, key) {
            if col.ends_with("date") && v.as_deref().is_some_and(|d| growth::date(d).is_none()) {
                return invalid("dates must be YYYY-MM-DD");
            }
            tx.execute(&format!("UPDATE goals SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    if let Some(d) = p.get("domain").and_then(|t| t.as_str()) {
        if !DOMAINS.contains(&d) {
            return invalid("unknown goal domain");
        }
        tx.execute("UPDATE goals SET domain=?2 WHERE id=?1", params![id, d])?;
    }
    for (col, key, allowed) in [("risk_level", "riskLevel", &["low", "medium", "high"][..]), ("confidence", "confidence", &["unknown", "low", "medium", "high"][..])] {
        if let Some(v) = p.get(key).and_then(|t| t.as_str()) {
            if !allowed.contains(&v) {
                return invalid(format!("unknown {key}"));
            }
            tx.execute(&format!("UPDATE goals SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    if let Some(v) = p.get("priority").and_then(|t| t.as_i64()) {
        tx.execute("UPDATE goals SET priority=?2 WHERE id=?1", params![id, v.clamp(1, 3)])?;
    }
    if let Some(v) = p.get("timeBudgetHoursPerWeek") {
        tx.execute("UPDATE goals SET time_budget_hours_per_week=?2 WHERE id=?1", params![id, v.as_f64()])?;
    }
    if let Some(v) = p.get("budgetLimitMinor") {
        tx.execute("UPDATE goals SET budget_limit_minor=?2 WHERE id=?1", params![id, v.as_i64()])?;
    }
    if let Some(s) = p.get("status").and_then(|t| t.as_str()) {
        if !matches!(s, "draft" | "active" | "paused" | "achieved" | "cancelled") {
            return invalid("unknown goal status");
        }
        if s == "achieved" {
            let success: String = tx.query_row("SELECT success_definition FROM goals WHERE id=?1", [id], |r| r.get(0))?;
            let blockers = growth::achievement_blockers(&requirements(&tx, id)?, &success);
            if !blockers.is_empty() {
                return invalid(format!("Cannot mark achieved yet: {}", blockers.join("; ")));
            }
        }
        tx.execute("UPDATE goals SET status=?2 WHERE id=?1", params![id, s])?;
    }
    if p.get("markReviewed").and_then(|v| v.as_bool()) == Some(true) {
        tx.execute("UPDATE goals SET last_reviewed_at=?2 WHERE id=?1", params![id, tasks::today(&tx)])?;
    }
    tx.commit()?;
    touch(conn, id)?;
    get_goal(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    get_goal(conn, id)?;
    conn.execute("UPDATE goals SET deleted_at=?2 WHERE id=?1", params![id, now()])?;
    search::remove(conn, id)
}

pub fn restore(conn: &Connection, id: &str) -> Result<Value> {
    conn.execute("UPDATE goals SET deleted_at=NULL WHERE id=?1", [id])?;
    index(conn, id)?;
    get_goal(conn, id)
}

pub fn delete_forever(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM goals WHERE id=?1 AND deleted_at IS NOT NULL", [id])?;
    search::remove(conn, id)
}

// ---------- metrics ----------

/// Tasks linked to a goal through its milestones or requirement allocations (each task once).
pub fn goal_task_ids(conn: &Connection, goal_id: &str) -> Result<Vec<String>> {
    let ids = conn
        .prepare(
            "SELECT t.id FROM tasks t WHERE t.deleted_at IS NULL AND (
               t.milestone_id IN (SELECT id FROM milestones WHERE goal_id=?1)
               OR t.id IN (SELECT l.task_id FROM task_requirement_links l JOIN goal_requirements r ON r.id=l.requirement_id WHERE r.goal_id=?1))
             ORDER BY t.due_date IS NULL, t.due_date, t.created_at",
        )?
        .query_map([goal_id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(ids)
}

fn effort(t: &tasks::Task) -> f64 {
    t.estimated_effort_minutes.map(|m| m as f64).filter(|m| *m > 0.0).unwrap_or(DEFAULT_EFFORT_MIN)
}

/// All four progress dimensions plus health, each with how it was computed.
pub fn metrics(conn: &Connection, goal_id: &str) -> Result<Value> {
    let g = get_goal(conn, goal_id)?;
    let today = today(conn);
    let reqs = requirements(conn, goal_id)?;
    let prep = growth::preparation(&reqs, today);
    let task_list: Vec<tasks::Task> = goal_task_ids(conn, goal_id)?.iter().map(|id| tasks::get(conn, id)).collect::<Result<_>>()?;
    let completion = growth::task_completion(&task_list.iter().map(|t| TaskInput { done: t.status == "done", effort: effort(t) }).collect::<Vec<_>>());
    let ms = milestones(conn, goal_id)?;
    let ms_inputs: Vec<MilestoneInput> = ms
        .iter()
        .map(|m| {
            let mine: Vec<&tasks::Task> = task_list.iter().filter(|t| t.milestone_id.as_deref() == m["id"].as_str()).collect();
            MilestoneInput {
                weight: m["weight"].as_f64().unwrap_or(1.0),
                status: m["status"].as_str().unwrap_or("").into(),
                tasks_done: mine.iter().filter(|t| t.status == "done").map(|t| effort(t)).sum(),
                tasks_total: mine.iter().map(|t| effort(t)).sum(),
            }
        })
        .collect();
    let roadmap = growth::roadmap_progress(&ms_inputs);
    let skills_out = goal_skills(conn, goal_id)?;
    let assessed: Vec<&Value> = skills_out.iter().filter(|s| s["currentLevel"].is_number()).collect();
    let skill_readiness = if assessed.is_empty() {
        None
    } else {
        Some(
            assessed.iter().map(|s| (s["currentLevel"].as_f64().unwrap() / s["targetLevel"].as_f64().unwrap_or(3.0).max(1.0)).min(1.0)).sum::<f64>()
                / assessed.len() as f64,
        )
    };
    let today_s = today.to_string();
    let overdue = task_list.iter().filter(|t| t.status != "done" && t.due_date.as_deref().is_some_and(|d| d < today_s.as_str())).count();
    let health = growth::goal_health(&HealthInput {
        status: g["status"].as_str().unwrap_or(""),
        today,
        start: g["startDate"].as_str().and_then(growth::date).or_else(|| g["createdAt"].as_str().and_then(growth::date)),
        target: g["targetDate"].as_str().and_then(growth::date),
        preparation: &prep,
        reqs: &reqs,
        overdue_tasks: overdue,
        last_reviewed_at: g["lastReviewedAt"].as_str().and_then(growth::date),
        success_definition: g["successDefinition"].as_str().unwrap_or(""),
    });
    let req_progress: Vec<Value> = reqs.iter().map(|r| serde_json::to_value(growth::requirement_progress(r, today)).unwrap()).collect();
    let unverified = req_progress.iter().filter(|r| matches!(r["verificationState"].as_str(), Some("unverified" | "outdated"))).count();
    let done = task_list.iter().filter(|t| t.status == "done").count();
    Ok(json!({
        "taskCompletion": { "value": completion, "done": done, "total": task_list.len(),
            "formula": "Σ effort × done ÷ Σ effort (estimated minutes; 60 when not estimated)",
            "note": "Shows work done in the plan, not readiness." },
        "roadmapProgress": { "value": roadmap, "milestones": ms.len(), "met": ms.iter().filter(|m| m["status"] == "met").count(),
            "formula": "Σ milestone weight × (1 if met, else share of its task effort done) ÷ Σ weight" },
        "preparation": { "value": prep.progress, "met": prep.met_count, "total": prep.total, "unknownCount": prep.unknown_count,
            "unknownWeight": prep.unknown_weight, "knownWeight": prep.known_weight, "requirements": req_progress, "unverified": unverified,
            "formula": "Σ weight × requirement progress ÷ Σ weight, over requirements with known progress; unknown ones are listed separately" },
        "skillReadiness": { "value": skill_readiness, "assessed": assessed.len(), "total": skills_out.len(), "skills": skills_out,
            "formula": "mean of (assessed level ÷ target level) over assessed skills — an estimate from evidence, not a true ability score" },
        "health": health,
        "achievementBlockers": growth::achievement_blockers(&reqs, g["successDefinition"].as_str().unwrap_or("")),
        "overdueTasks": overdue,
    }))
}

pub fn goal_skills(conn: &Connection, goal_id: &str) -> Result<Vec<Value>> {
    let rows: Vec<(String, String, i64)> = conn
        .prepare("SELECT s.id, s.name, gs.target_level FROM goal_skills gs JOIN skills s ON s.id=gs.skill_id WHERE gs.goal_id=?1 AND s.deleted_at IS NULL ORDER BY s.name")?
        .query_map([goal_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let today = today(conn);
    rows.into_iter()
        .map(|(id, name, target)| {
            let evs: Vec<EvidenceInput> = skills::evidence_inputs(conn, &id)?;
            let r = growth::skill_readiness(&evs, today);
            Ok(json!({ "skillId": id, "name": name, "targetLevel": target, "currentLevel": r.current_level, "confidence": r.confidence,
                       "lastAssessedAt": r.last_assessed_at, "stale": r.stale, "evidenceCount": r.evidence_count,
                       "gap": r.current_level.map(|c| (target - c).max(0)) }))
        })
        .collect()
}

pub fn set_goal_skill(conn: &Connection, goal_id: &str, skill_id: &str, target_level: Option<i64>) -> Result<()> {
    match target_level {
        Some(l) => {
            conn.execute(
                "INSERT INTO goal_skills(goal_id, skill_id, target_level) VALUES (?1,?2,?3) ON CONFLICT(goal_id, skill_id) DO UPDATE SET target_level=excluded.target_level",
                params![goal_id, skill_id, l.clamp(0, 5)],
            )?;
        }
        None => {
            conn.execute("DELETE FROM goal_skills WHERE goal_id=?1 AND skill_id=?2", params![goal_id, skill_id])?;
        }
    }
    touch(conn, goal_id)
}

pub fn list(conn: &Connection, domain: Option<&str>, include_closed: bool) -> Result<Vec<Value>> {
    let ids: Vec<String> = conn
        .prepare(
            "SELECT id FROM goals WHERE deleted_at IS NULL AND (?1 IS NULL OR domain=?1)
             AND (?2 = 1 OR status IN ('draft','active','paused')) ORDER BY status<>'active', priority, target_date IS NULL, target_date",
        )?
        .query_map(params![domain, include_closed as i64], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    ids.iter()
        .map(|id| {
            let mut g = get_goal(conn, id)?;
            let m = metrics(conn, id)?;
            g["metrics"] = json!({
                "taskCompletion": m["taskCompletion"]["value"], "roadmapProgress": m["roadmapProgress"]["value"],
                "preparation": m["preparation"]["value"], "unknownCount": m["preparation"]["unknownCount"], "unverified": m["preparation"]["unverified"],
                "skillReadiness": m["skillReadiness"]["value"], "health": m["health"],
                "tasksDone": m["taskCompletion"]["done"], "tasksTotal": m["taskCompletion"]["total"],
                "requirementsMet": m["preparation"]["met"], "requirementsTotal": m["preparation"]["total"],
            });
            Ok(g)
        })
        .collect()
}

/// Everything the Goal Detail screen needs in one call.
pub fn detail(conn: &Connection, id: &str) -> Result<Value> {
    let goal = get_goal(conn, id)?;
    let reqs: Vec<Value> = conn
        .prepare("SELECT id FROM goal_requirements WHERE goal_id=?1 ORDER BY position")?
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .iter()
        .map(|rid| {
            let mut r = requirement_json(conn, rid)?;
            r["evidence"] = json!(skills::evidence_for_requirement(conn, rid)?);
            r["taskCount"] = json!(conn.query_row("SELECT COUNT(*) FROM task_requirement_links l JOIN tasks t ON t.id=l.task_id WHERE l.requirement_id=?1 AND t.deleted_at IS NULL", [rid], |x| x.get::<_, i64>(0))?);
            Ok(r)
        })
        .collect::<Result<_>>()?;
    let task_list: Vec<tasks::Task> = goal_task_ids(conn, id)?.iter().map(|t| tasks::get(conn, t)).collect::<Result<_>>()?;
    let drafts = list_drafts(conn, Some(id))?;
    let finance = super::finance::goal_finance(conn, id)?;
    Ok(json!({
        "goal": goal, "requirements": reqs, "milestones": milestones(conn, id)?, "tasks": task_list,
        "metrics": metrics(conn, id)?, "drafts": drafts, "finance": finance,
    }))
}

// ---------- drafts from AI clients ----------

/// Stores a proposed breakdown; nothing is created until the user applies it.
pub fn create_draft(conn: &Connection, goal_id: &str, payload: &Value, source: &str) -> Result<Value> {
    get_goal(conn, goal_id)?;
    let reqs = payload.get("requirements").and_then(|r| r.as_array()).ok_or_else(|| AppError::Invalid("requirements array is required".into()))?;
    if reqs.is_empty() || reqs.len() > 30 {
        return invalid("propose between 1 and 30 requirements");
    }
    let mut count = 0;
    for r in reqs {
        if r.get("title").and_then(|t| t.as_str()).map(str::trim).unwrap_or("").is_empty() {
            return invalid("every requirement needs a title");
        }
        for m in r.get("milestones").and_then(|m| m.as_array()).into_iter().flatten() {
            count += 1 + m.get("tasks").and_then(|t| t.as_array()).map(|t| t.len()).unwrap_or(0);
        }
    }
    if count > 400 {
        return invalid("draft is too large (max 400 milestones + tasks)");
    }
    let id = new_id();
    conn.execute(
        "INSERT INTO drafts(id, kind, target_id, payload_json, source, created_at) VALUES (?1,'goal_breakdown',?2,?3,?4,?5)",
        params![id, goal_id, payload.to_string(), source, now()],
    )?;
    Ok(json!({ "draftId": id, "status": "pending", "message": "Saved as a draft. The user reviews and applies it in Personal Workspace; nothing was changed yet." }))
}

pub fn list_drafts(conn: &Connection, goal_id: Option<&str>) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, target_id, payload_json, source, created_at FROM drafts WHERE status='pending' AND kind='goal_breakdown' AND (?1 IS NULL OR target_id=?1) ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([goal_id], |r| {
        Ok(json!({ "id": r.get::<_, String>(0)?, "kind": r.get::<_, String>(1)?, "targetId": r.get::<_, Option<String>>(2)?,
                   "payload": serde_json::from_str::<Value>(&r.get::<_, String>(3)?).unwrap_or(Value::Null), "source": r.get::<_, String>(4)?, "createdAt": r.get::<_, String>(5)? }))
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn resolve_draft(conn: &Connection, draft_id: &str, apply: bool) -> Result<Value> {
    let (goal_id, payload, status): (String, String, String) = conn
        .query_row("SELECT target_id, payload_json, status FROM drafts WHERE id=?1", [draft_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?
        .ok_or_else(|| AppError::NotFound("draft".into()))?;
    if status != "pending" {
        return invalid("this draft was already handled");
    }
    let mut created = json!({ "requirements": 0, "milestones": 0, "tasks": 0 });
    if apply {
        let payload: Value = serde_json::from_str(&payload)?;
        let tx = crate::infrastructure::database::begin(conn)?;
        for r in payload["requirements"].as_array().into_iter().flatten() {
            let mut rp = Map::new();
            for k in ["title", "description", "type", "critical", "weight", "targetValue", "unit", "deadline", "verificationSource"] {
                if let Some(v) = r.get(k) {
                    rp.insert(k.into(), v.clone());
                }
            }
            let req = add_requirement(&tx, &goal_id, &Value::Object(rp))?;
            created["requirements"] = json!(created["requirements"].as_i64().unwrap() + 1);
            for m in r["milestones"].as_array().into_iter().flatten() {
                let ms = add_milestone(&tx, &goal_id, &json!({ "title": m["title"], "phase": m.get("phase").cloned().unwrap_or(json!("")), "dueDate": m.get("dueDate").cloned().unwrap_or(Value::Null), "requirementId": req["id"], "acceptanceCriteria": m.get("acceptanceCriteria").cloned().unwrap_or(json!("")) }))?;
                created["milestones"] = json!(created["milestones"].as_i64().unwrap() + 1);
                for t in m["tasks"].as_array().into_iter().flatten() {
                    let title = t.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
                    if title.is_empty() {
                        continue;
                    }
                    add_milestone_task(&tx, ms["id"].as_str().unwrap(), &json!({ "title": title, "dueDate": t.get("dueDate").cloned().unwrap_or(Value::Null), "estimatedEffortMinutes": t.get("estimatedMinutes").cloned().unwrap_or(Value::Null), "description": t.get("description").cloned().unwrap_or(json!("")) }))?;
                    created["tasks"] = json!(created["tasks"].as_i64().unwrap() + 1);
                }
            }
        }
        tx.execute("UPDATE drafts SET status='applied', resolved_at=?2 WHERE id=?1", params![draft_id, now()])?;
        tx.commit()?;
    } else {
        conn.execute("UPDATE drafts SET status='discarded', resolved_at=?2 WHERE id=?1", params![draft_id, now()])?;
    }
    Ok(json!({ "applied": apply, "created": created }))
}

// ---------- illustrative example (PRD §6 / §21) ----------

/// Creates the CEDT CU preparation scenario. Criteria are placeholders and marked unverified on purpose:
/// the user must replace them with the official announcement for their round and year.
pub fn create_example(conn: &Connection) -> Result<Value> {
    let today = tasks::today(conn);
    let d = |n: i64| (growth::date(&today).unwrap() + chrono::Duration::days(n)).to_string();
    let g = create(conn, &json!({
        "title": "เตรียมตัวสมัคร CEDT CU (ตัวอย่าง)", "domain": "education", "status": "active", "priority": 1,
        "targetDate": d(180), "timeBudgetHoursPerWeek": 10,
        "motivation": "ตัวอย่างสำหรับทดลองระบบ — เกณฑ์ทั้งหมดเป็นข้อมูลสมมติ",
        "successDefinition": "ผ่านข้อกำหนดตามประกาศอย่างเป็นทางการ + ยื่นเอกสารครบตามกำหนด + ทราบผลรับสมัครจริง",
    }))?;
    conn.execute("UPDATE goals SET is_example=1 WHERE id=?1", [g["id"].as_str().unwrap()])?;
    let gid = g["id"].as_str().unwrap().to_string();
    let plan = [
        ("R1 ความพร้อมคณิตศาสตร์ (สมมติ)", "score", 70.0, "คะแนน", vec![
            ("Build foundations", "พื้นฐานฟังก์ชันและพีชคณิต", vec!["อ่านบทฟังก์ชัน", "ทำโจทย์ฟังก์ชัน 20 ข้อ"]),
            ("Mock tests", "Mock test คณิต 2 ชุด", vec!["ทำ Mock Test ชุด A แบบจับเวลา", "วิเคราะห์ข้อผิดพลาดรายหัวข้อ"]),
        ]),
        ("R2 ความพร้อมภาษาอังกฤษ (สมมติ)", "score", 70.0, "คะแนน", vec![
            ("Build foundations", "Reading & vocabulary", vec!["อ่านบทความวิชาการ 3 บทความ", "ทบทวนคำศัพท์ 100 คำ"]),
            ("Mock tests", "Timed practice", vec!["ทำข้อสอบ Reading จับเวลา"]),
        ]),
        ("R3 Computing / Problem solving (สมมติ)", "portfolio", 0.0, "", vec![
            ("Build foundations", "Programming basics → algorithms", vec!["แก้โจทย์โค้ด 10 ข้อ", "เขียนอธิบาย Algorithm ที่ใช้"]),
            ("Mini project", "Mini project พร้อม README", vec!["ทำ Mini Project และ push ขึ้น Git"]),
        ]),
        ("R4 เอกสารและขั้นตอนสมัคร", "external_verification", 0.0, "", vec![
            ("Verify requirements", "ตรวจประกาศรับสมัครล่าสุด", vec!["เปิดประกาศรับสมัครของรอบที่เลือก", "บันทึกเงื่อนไข เอกสาร และกำหนดเวลา พร้อมลิงก์อ้างอิง"]),
            ("Application logistics", "เตรียมและยื่นเอกสาร", vec!["เตรียมเอกสารและตรวจความครบถ้วน", "บันทึกหลักฐานการยื่นสมัคร"]),
        ]),
    ];
    for (i, (title, rtype, target, unit, mss)) in plan.into_iter().enumerate() {
        let req = add_requirement(conn, &gid, &json!({
            "title": title, "type": rtype, "weight": 1, "critical": true, "unit": unit,
            "targetValue": if target > 0.0 { json!(target) } else { Value::Null },
            "description": "ข้อมูลสมมติ — แทนด้วยเกณฑ์จริงจากประกาศอย่างเป็นทางการ",
            "verificationSource": if i == 3 { "ใส่ URL ประกาศรับสมัครอย่างเป็นทางการ" } else { "" },
        }))?;
        for (j, (phase, mtitle, tasks_)) in mss.into_iter().enumerate() {
            let ms = add_milestone(conn, &gid, &json!({ "title": mtitle, "phase": phase, "requirementId": req["id"], "dueDate": d(30 + 45 * j as i64) }))?;
            for (k, t) in tasks_.into_iter().enumerate() {
                add_milestone_task(conn, ms["id"].as_str().unwrap(), &json!({ "title": t, "dueDate": if j == 0 && k == 0 { json!(today) } else { json!(d(7 + 14 * (j as i64 + k as i64))) }, "estimatedEffortMinutes": 60 }))?;
            }
        }
    }
    get_goal(conn, &gid)
}

/// Compact list for pickers: active/draft/paused goals with their requirements and milestones.
pub fn options(conn: &Connection) -> Result<Vec<Value>> {
    let goals: Vec<(String, String)> = conn
        .prepare("SELECT id, title FROM goals WHERE deleted_at IS NULL AND status IN ('draft','active','paused') ORDER BY priority, title")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    goals
        .into_iter()
        .map(|(id, title)| {
            let reqs: Vec<Value> = conn
                .prepare("SELECT id, title FROM goal_requirements WHERE goal_id=?1 ORDER BY position")?
                .query_map([&id], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)? })))?
                .collect::<std::result::Result<_, _>>()?;
            let ms: Vec<Value> = conn
                .prepare("SELECT id, title, phase, requirement_id FROM milestones WHERE goal_id=?1 ORDER BY position")?
                .query_map([&id], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "phase": r.get::<_, String>(2)?, "requirementId": r.get::<_, Option<String>>(3)? })))?
                .collect::<std::result::Result<_, _>>()?;
            Ok(json!({ "id": id, "title": title, "requirements": reqs, "milestones": ms }))
        })
        .collect()
}

/// Orphan detector: items that are not connected to anything that measures progress.
pub fn orphans(conn: &Connection) -> Result<Value> {
    let list = |sql: &str| -> Result<Vec<Value>> {
        Ok(conn.prepare(sql)?.query_map([], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)? })))?.collect::<std::result::Result<_, _>>()?)
    };
    Ok(json!({
        "tasksWithoutGoalOrProject": list("SELECT id, title FROM tasks WHERE deleted_at IS NULL AND status<>'done' AND project_id IS NULL AND milestone_id IS NULL
            AND id NOT IN (SELECT task_id FROM task_requirement_links) ORDER BY created_at DESC LIMIT 50")?,
        "goalsWithoutRequirements": list("SELECT id, title FROM goals WHERE deleted_at IS NULL AND status IN ('draft','active') AND id NOT IN (SELECT goal_id FROM goal_requirements)")?,
        "goalsWithoutSuccessDefinition": list("SELECT id, title FROM goals WHERE deleted_at IS NULL AND status='active' AND trim(success_definition)=''")?,
        "requirementsWithoutWork": list("SELECT g.id, r.title || ' — ' || g.title FROM goal_requirements r JOIN goals g ON g.id=r.goal_id WHERE g.deleted_at IS NULL AND g.status='active' AND r.satisfaction_status<>'met'
            AND r.id NOT IN (SELECT requirement_id FROM task_requirement_links) AND r.id NOT IN (SELECT requirement_id FROM milestones WHERE requirement_id IS NOT NULL)")?,
        "skillsWithoutEvidence": list("SELECT id, name FROM skills WHERE deleted_at IS NULL AND id NOT IN (SELECT skill_id FROM skill_evidence)")?,
    }))
}

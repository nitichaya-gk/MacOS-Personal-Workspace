//! Learning hub: courses → topics (with prerequisites, skill links, spaced review) → resources.
//! Review scheduling is a simple SM-2 variant; it plans *when to review*, it does not grade ability.

use super::tasks;
use crate::domain::growth;
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use chrono::Duration;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{json, Value};

fn course_row(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "provider": r.get::<_, String>(2)?, "url": r.get::<_, String>(3)?,
        "goalId": r.get::<_, Option<String>>(4)?, "status": r.get::<_, String>(5)?, "startDate": r.get::<_, Option<String>>(6)?,
        "endDate": r.get::<_, Option<String>>(7)?, "notes": r.get::<_, String>(8)?, "goalTitle": r.get::<_, Option<String>>(9)?,
    }))
}
const COURSE_SELECT: &str = "SELECT c.id, c.title, c.provider, c.url, c.goal_id, c.status, c.start_date, c.end_date, c.notes, g.title
    FROM learning_courses c LEFT JOIN goals g ON g.id=c.goal_id";

fn topic_row(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "courseId": r.get::<_, Option<String>>(1)?, "title": r.get::<_, String>(2)?, "skillId": r.get::<_, Option<String>>(3)?,
        "prerequisites": serde_json::from_str::<Value>(&r.get::<_, String>(4)?).unwrap_or(json!([])), "status": r.get::<_, String>(5)?,
        "position": r.get::<_, f64>(6)?, "ease": r.get::<_, f64>(7)?, "intervalDays": r.get::<_, i64>(8)?, "reviewDue": r.get::<_, Option<String>>(9)?,
        "lastReviewedAt": r.get::<_, Option<String>>(10)?, "errorCount": r.get::<_, i64>(11)?, "skillName": r.get::<_, Option<String>>(12)?,
        "courseTitle": r.get::<_, Option<String>>(13)?,
    }))
}
const TOPIC_SELECT: &str = "SELECT t.id, t.course_id, t.title, t.skill_id, t.prerequisites, t.status, t.position, t.ease, t.interval_days, t.review_due,
    t.last_reviewed_at, t.error_count, s.name, c.title FROM learning_topics t LEFT JOIN skills s ON s.id=t.skill_id LEFT JOIN learning_courses c ON c.id=t.course_id";

pub fn overview(conn: &Connection) -> Result<Value> {
    let courses: Vec<Value> = conn.prepare(&format!("{COURSE_SELECT} WHERE c.deleted_at IS NULL ORDER BY c.status='done', c.created_at"))?.query_map([], course_row)?.collect::<std::result::Result<_, _>>()?;
    let topics: Vec<Value> = conn.prepare(&format!("{TOPIC_SELECT} WHERE c.id IS NULL OR c.deleted_at IS NULL ORDER BY t.position"))?.query_map([], topic_row)?.collect::<std::result::Result<_, _>>()?;
    let resources: Vec<Value> = conn
        .prepare("SELECT id, course_id, topic_id, title, kind, url, page_id, done FROM learning_resources ORDER BY created_at")?
        .query_map([], |r| {
            Ok(json!({ "id": r.get::<_, String>(0)?, "courseId": r.get::<_, Option<String>>(1)?, "topicId": r.get::<_, Option<String>>(2)?, "title": r.get::<_, String>(3)?,
                       "kind": r.get::<_, String>(4)?, "url": r.get::<_, String>(5)?, "pageId": r.get::<_, Option<String>>(6)?, "done": r.get::<_, i64>(7)? != 0 }))
        })?
        .collect::<std::result::Result<_, _>>()?;
    let today = tasks::today(conn);
    let due: Vec<&Value> = topics.iter().filter(|t| t["reviewDue"].as_str().is_some_and(|d| d <= today.as_str())).collect();
    // Error heatmap: topics with the most failed reviews.
    let mut weak: Vec<&Value> = topics.iter().filter(|t| t["errorCount"].as_i64().unwrap_or(0) > 0).collect();
    weak.sort_by_key(|t| -t["errorCount"].as_i64().unwrap_or(0));
    Ok(json!({ "courses": courses, "topics": topics, "resources": resources, "reviewsDue": due, "weakTopics": weak.into_iter().take(8).collect::<Vec<_>>(), "today": today }))
}

fn s(p: &Value, k: &str) -> Option<Option<String>> {
    p.get(k).map(|v| v.as_str().map(str::trim).filter(|x| !x.is_empty()).map(String::from))
}

pub fn save_course(conn: &Connection, id: Option<&str>, p: &Value) -> Result<Value> {
    let id = match id {
        Some(id) => id.to_string(),
        None => {
            let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
            if title.is_empty() {
                return invalid("course title is required");
            }
            let id = new_id();
            conn.execute("INSERT INTO learning_courses(id, title, created_at, updated_at) VALUES (?1,?2,?3,?3)", params![id, title, now()])?;
            id
        }
    };
    for (col, key) in [("title", "title"), ("provider", "provider"), ("url", "url"), ("notes", "notes")] {
        if let Some(v) = s(p, key) {
            if col == "title" && v.is_none() {
                return invalid("course title is required");
            }
            conn.execute(&format!("UPDATE learning_courses SET {col}=?2 WHERE id=?1"), params![id, v.unwrap_or_default()])?;
        }
    }
    for (col, key) in [("goal_id", "goalId"), ("start_date", "startDate"), ("end_date", "endDate")] {
        if let Some(v) = s(p, key) {
            conn.execute(&format!("UPDATE learning_courses SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    if let Some(st) = p.get("status").and_then(|v| v.as_str()) {
        if !matches!(st, "planned" | "active" | "paused" | "done") {
            return invalid("unknown course status");
        }
        conn.execute("UPDATE learning_courses SET status=?2 WHERE id=?1", params![id, st])?;
    }
    conn.execute("UPDATE learning_courses SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    conn.query_row(&format!("{COURSE_SELECT} WHERE c.id=?1"), [&id], course_row).optional()?.ok_or_else(|| AppError::NotFound("course".into()))
}

pub fn delete_course(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE learning_courses SET deleted_at=?2 WHERE id=?1", params![id, now()])?;
    Ok(())
}

fn get_topic(conn: &Connection, id: &str) -> Result<Value> {
    conn.query_row(&format!("{TOPIC_SELECT} WHERE t.id=?1"), [id], topic_row).optional()?.ok_or_else(|| AppError::NotFound("topic".into()))
}

/// Would making `id` depend on `on` create a prerequisite cycle?
fn prereq_cycle(conn: &Connection, id: &str, on: &[String]) -> Result<bool> {
    let mut stack: Vec<String> = on.to_vec();
    let mut seen = std::collections::HashSet::new();
    while let Some(cur) = stack.pop() {
        if cur == id {
            return Ok(true);
        }
        if !seen.insert(cur.clone()) {
            continue;
        }
        if let Some(p) = conn.query_row("SELECT prerequisites FROM learning_topics WHERE id=?1", [&cur], |r| r.get::<_, String>(0)).optional()? {
            let next: Vec<String> = serde_json::from_str(&p).unwrap_or_default();
            stack.extend(next);
        }
    }
    Ok(false)
}

pub fn save_topic(conn: &Connection, id: Option<&str>, p: &Value) -> Result<Value> {
    let id = match id {
        Some(id) => id.to_string(),
        None => {
            let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
            if title.is_empty() {
                return invalid("topic title is required");
            }
            let id = new_id();
            let pos: f64 = conn.query_row("SELECT COALESCE(MAX(position)+1,0) FROM learning_topics", [], |r| r.get(0))?;
            conn.execute("INSERT INTO learning_topics(id, title, position, created_at, updated_at) VALUES (?1,?2,?3,?4,?4)", params![id, title, pos, now()])?;
            id
        }
    };
    if let Some(v) = s(p, "title") {
        conn.execute("UPDATE learning_topics SET title=?2 WHERE id=?1", params![id, v.ok_or_else(|| AppError::Invalid("topic title is required".into()))?])?;
    }
    for (col, key) in [("course_id", "courseId"), ("skill_id", "skillId")] {
        if let Some(v) = s(p, key) {
            conn.execute(&format!("UPDATE learning_topics SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    if let Some(st) = p.get("status").and_then(|v| v.as_str()) {
        if !matches!(st, "not_started" | "learning" | "reviewing" | "mastered") {
            return invalid("unknown topic status");
        }
        conn.execute("UPDATE learning_topics SET status=?2 WHERE id=?1", params![id, st])?;
        // Starting a review cycle: first review tomorrow.
        if st == "reviewing" {
            let due = growth::date(&tasks::today(conn)).unwrap() + Duration::days(1);
            conn.execute("UPDATE learning_topics SET review_due=COALESCE(review_due, ?2) WHERE id=?1", params![id, due.to_string()])?;
        }
    }
    if let Some(pre) = p.get("prerequisites").and_then(|v| v.as_array()) {
        let ids: Vec<String> = pre.iter().filter_map(|x| x.as_str().map(String::from)).filter(|x| x != &id).collect();
        if prereq_cycle(conn, &id, &ids)? {
            return invalid("those prerequisites would form a loop");
        }
        conn.execute("UPDATE learning_topics SET prerequisites=?2 WHERE id=?1", params![id, json!(ids).to_string()])?;
    }
    conn.execute("UPDATE learning_topics SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    get_topic(conn, &id)
}

pub fn delete_topic(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM learning_topics WHERE id=?1", [id])?;
    Ok(())
}

/// Records a review with recall quality 0–5 and schedules the next one (SM-2 style).
pub fn review(conn: &Connection, id: &str, quality: i64) -> Result<Value> {
    if !(0..=5).contains(&quality) {
        return invalid("quality must be 0–5");
    }
    let t = get_topic(conn, id)?;
    let mut ease = t["ease"].as_f64().unwrap_or(2.5);
    let mut interval = t["intervalDays"].as_i64().unwrap_or(0);
    let q = quality as f64;
    if quality < 3 {
        interval = 1;
        conn.execute("UPDATE learning_topics SET error_count=error_count+1 WHERE id=?1", [id])?;
    } else {
        interval = match interval {
            0 => 1,
            1 => 3,
            n => ((n as f64) * ease).round() as i64,
        };
    }
    ease = (ease + 0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02)).max(1.3);
    let today = growth::date(&tasks::today(conn)).unwrap();
    let due = today + Duration::days(interval.min(365));
    let status = if interval >= 30 { "mastered" } else { "reviewing" };
    conn.execute(
        "UPDATE learning_topics SET ease=?2, interval_days=?3, review_due=?4, last_reviewed_at=?5, status=?6, updated_at=?7 WHERE id=?1",
        params![id, ease, interval, due.to_string(), today.to_string(), status, now()],
    )?;
    get_topic(conn, id)
}

pub fn save_resource(conn: &Connection, id: Option<&str>, p: &Value) -> Result<Value> {
    let id = match id {
        Some(id) => id.to_string(),
        None => {
            let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
            if title.is_empty() {
                return invalid("resource title is required");
            }
            let id = new_id();
            conn.execute("INSERT INTO learning_resources(id, title, created_at) VALUES (?1,?2,?3)", params![id, title, now()])?;
            id
        }
    };
    for (col, key) in [("course_id", "courseId"), ("topic_id", "topicId"), ("page_id", "pageId")] {
        if let Some(v) = s(p, key) {
            conn.execute(&format!("UPDATE learning_resources SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    for (col, key) in [("title", "title"), ("url", "url")] {
        if let Some(v) = s(p, key) {
            conn.execute(&format!("UPDATE learning_resources SET {col}=?2 WHERE id=?1"), params![id, v.unwrap_or_default()])?;
        }
    }
    if let Some(k) = p.get("kind").and_then(|v| v.as_str()) {
        if !matches!(k, "link" | "book" | "video" | "exercise" | "note" | "file") {
            return invalid("unknown resource type");
        }
        conn.execute("UPDATE learning_resources SET kind=?2 WHERE id=?1", params![id, k])?;
    }
    if let Some(d) = p.get("done").and_then(|v| v.as_bool()) {
        conn.execute("UPDATE learning_resources SET done=?2 WHERE id=?1", params![id, d as i64])?;
    }
    Ok(json!({ "id": id }))
}

pub fn delete_resource(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM learning_resources WHERE id=?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::application::workspace::Workspace;
    use serde_json::json;

    #[test]
    fn review_schedule_grows_and_resets() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::open(&dir.path().join("w")).unwrap();
        let c = &ws.conn;
        let t = super::save_topic(c, None, &json!({ "title": "ฟังก์ชัน" })).unwrap();
        let id = t["id"].as_str().unwrap();
        let a = super::review(c, id, 5).unwrap();
        assert_eq!(a["intervalDays"], 1);
        let b = super::review(c, id, 4).unwrap();
        assert_eq!(b["intervalDays"], 3);
        let c2 = super::review(c, id, 4).unwrap();
        assert!(c2["intervalDays"].as_i64().unwrap() >= 7);
        let d = super::review(c, id, 1).unwrap();
        assert_eq!(d["intervalDays"], 1);
        assert_eq!(d["errorCount"], 1);
        // Prerequisite loops are refused.
        let t2 = super::save_topic(c, None, &json!({ "title": "อนุพันธ์", "prerequisites": [id] })).unwrap();
        assert!(super::save_topic(c, Some(id), &json!({ "prerequisites": [t2["id"]] })).is_err());
    }
}

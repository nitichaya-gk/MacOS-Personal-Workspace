//! Personal-growth rules: goal progress dimensions, goal health, skill readiness, habit streaks and money.
//!
//! The four progress dimensions are deliberately computed and returned separately (never summed):
//! task completion, roadmap progress, requirement satisfaction ("preparation") and skill readiness.
//! Unknown values are reported as unknown coverage rather than silently counted as 0.

use chrono::{Datelike, Duration, NaiveDate};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

pub fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.get(0..10)?, "%Y-%m-%d").ok()
}

// ---------- requirements ----------

#[derive(Clone, Debug, Default)]
pub struct ReqInput {
    pub id: String,
    pub title: String,
    pub rtype: String,
    pub critical: bool,
    pub weight: f64,
    pub target: Option<f64>,
    pub current: Option<f64>,
    pub satisfaction: String,
    pub verification: String,
    pub verification_source: String,
    pub source_checked_at: Option<String>,
    pub deadline: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReqProgress {
    pub id: String,
    /// None = unknown (no data yet); never coerced to 0.
    pub progress: Option<f64>,
    pub explanation: String,
    pub needs_verification: bool,
    pub verification_state: String,
}

/// Criteria that come from an outside authority (announcement, official document) must be verified.
pub fn needs_verification(r: &ReqInput) -> bool {
    matches!(r.rtype.as_str(), "external_verification" | "document") || !r.verification_source.trim().is_empty()
}

pub const VERIFICATION_MAX_AGE_DAYS: i64 = 90;

pub fn verification_state(r: &ReqInput, today: NaiveDate) -> String {
    if !needs_verification(r) {
        return "not_needed".into();
    }
    match (r.verification.as_str(), r.source_checked_at.as_deref().and_then(date)) {
        ("verified", Some(d)) if (today - d).num_days() > VERIFICATION_MAX_AGE_DAYS => "outdated".into(),
        ("verified", _) => "verified".into(),
        (other, _) => other.to_string(),
    }
}

pub fn requirement_progress(r: &ReqInput, today: NaiveDate) -> ReqProgress {
    let (progress, explanation) = match r.satisfaction.as_str() {
        "met" => (Some(1.0), "Marked met".to_string()),
        "unknown" => (None, "Status unknown".to_string()),
        _ => match (r.rtype.as_str(), r.target, r.current) {
            ("numeric" | "score", Some(t), Some(c)) if t > 0.0 => {
                let p = (c / t).clamp(0.0, 1.0);
                (Some(p), format!("{c} of target {t} → {:.0}%", p * 100.0))
            }
            ("numeric" | "score", _, None) => (None, "No measured value yet".to_string()),
            ("numeric" | "score", _, _) => (None, "No target value set".to_string()),
            // Binary-style requirements count only when met.
            _ => (Some(0.0), "Counts as 0 until met (binary)".to_string()),
        },
    };
    ReqProgress { id: r.id.clone(), progress, explanation, needs_verification: needs_verification(r), verification_state: verification_state(r, today) }
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Preparation {
    /// Weighted progress over requirements with known progress.
    pub progress: Option<f64>,
    pub known_weight: f64,
    pub unknown_weight: f64,
    pub unknown_count: usize,
    pub met_count: usize,
    pub total: usize,
}

/// GoalPreparationProgress = Σ wᵢ·pᵢ / Σ wᵢ (over requirements whose progress is known).
pub fn preparation(reqs: &[ReqInput], today: NaiveDate) -> Preparation {
    let mut out = Preparation { total: reqs.len(), ..Default::default() };
    let mut sum = 0.0;
    for r in reqs {
        let p = requirement_progress(r, today);
        if r.satisfaction == "met" {
            out.met_count += 1;
        }
        match p.progress {
            Some(v) => {
                out.known_weight += r.weight;
                sum += r.weight * v;
            }
            None => {
                out.unknown_weight += r.weight;
                out.unknown_count += 1;
            }
        }
    }
    out.progress = (out.known_weight > 0.0).then(|| sum / out.known_weight);
    out
}

/// A goal can be marked achieved only when every critical requirement is met and success is defined.
pub fn achievement_blockers(reqs: &[ReqInput], success_definition: &str) -> Vec<String> {
    let mut out = vec![];
    if success_definition.trim().is_empty() {
        out.push("No measurable success definition".to_string());
    }
    if reqs.is_empty() {
        out.push("No requirements defined".to_string());
    }
    for r in reqs.iter().filter(|r| r.critical && r.satisfaction != "met") {
        out.push(format!("Critical requirement not met: {}", r.title));
    }
    out
}

// ---------- tasks & roadmap ----------

#[derive(Clone, Debug)]
pub struct TaskInput {
    pub done: bool,
    /// Estimated effort in minutes (or 1 when unknown).
    pub effort: f64,
}

/// TaskCompletionRate = Σ effortⱼ·doneⱼ / Σ effortⱼ
pub fn task_completion(tasks: &[TaskInput]) -> Option<f64> {
    let total: f64 = tasks.iter().map(|t| t.effort.max(0.0)).sum();
    (total > 0.0).then(|| tasks.iter().filter(|t| t.done).map(|t| t.effort.max(0.0)).sum::<f64>() / total)
}

#[derive(Clone, Debug)]
pub struct MilestoneInput {
    pub weight: f64,
    pub status: String,
    pub tasks_done: f64,
    pub tasks_total: f64,
}

/// Roadmap progress = Σ milestone weight · (1 if met, else share of its task effort done) / Σ weight.
pub fn roadmap_progress(ms: &[MilestoneInput]) -> Option<f64> {
    let total: f64 = ms.iter().map(|m| m.weight).sum();
    if total <= 0.0 {
        return None;
    }
    let sum: f64 = ms
        .iter()
        .map(|m| {
            let p = if m.status == "met" { 1.0 } else if m.tasks_total > 0.0 { m.tasks_done / m.tasks_total } else { 0.0 };
            m.weight * p
        })
        .sum();
    Some(sum / total)
}

// ---------- goal health ----------

pub struct HealthInput<'a> {
    pub status: &'a str,
    pub today: NaiveDate,
    pub start: Option<NaiveDate>,
    pub target: Option<NaiveDate>,
    pub preparation: &'a Preparation,
    pub reqs: &'a [ReqInput],
    pub overdue_tasks: usize,
    pub last_reviewed_at: Option<NaiveDate>,
    pub success_definition: &'a str,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// on_track | at_risk | blocked | needs_review | inactive | achieved
    pub state: String,
    pub reasons: Vec<String>,
}

/// Rules-based health. It describes the plan, never the chance of being admitted or succeeding.
pub fn goal_health(h: &HealthInput) -> Health {
    match h.status {
        "achieved" => return Health { state: "achieved".into(), reasons: vec![] },
        "paused" | "cancelled" | "draft" => return Health { state: "inactive".into(), reasons: vec![format!("Goal is {}", h.status)] },
        _ => {}
    }
    let mut blocked = vec![];
    let mut risk = vec![];
    let mut review = vec![];
    for r in h.reqs {
        if r.satisfaction == "blocked" {
            blocked.push(format!("Requirement blocked: {}", r.title));
        }
        if r.critical && r.satisfaction != "met" {
            if let Some(d) = r.deadline.as_deref().and_then(date) {
                if d < h.today {
                    risk.push(format!("Deadline passed for “{}”", r.title));
                }
            }
        }
        match verification_state(r, h.today).as_str() {
            "unverified" => review.push(format!("Criteria not verified: {}", r.title)),
            "outdated" => review.push(format!("Criteria checked more than {VERIFICATION_MAX_AGE_DAYS} days ago: {}", r.title)),
            _ => {}
        }
    }
    if let Some(t) = h.target {
        if t < h.today {
            risk.push("Target date has passed".into());
        } else if let (Some(s), Some(p)) = (h.start, h.preparation.progress) {
            let span = (t - s).num_days().max(1) as f64;
            let elapsed = ((h.today - s).num_days().max(0) as f64 / span).min(1.0);
            if elapsed - p > 0.25 {
                risk.push(format!("{:.0}% of the time used but preparation at {:.0}%", elapsed * 100.0, p * 100.0));
            }
        }
    }
    if h.overdue_tasks > 0 {
        risk.push(format!("{} overdue task(s)", h.overdue_tasks));
    }
    if h.success_definition.trim().is_empty() {
        review.push("Add a measurable success definition".into());
    }
    if h.reqs.is_empty() {
        review.push("Add requirements so progress can be measured".into());
    }
    match h.last_reviewed_at {
        Some(d) if (h.today - d).num_days() > 14 => review.push("Not reviewed for more than 2 weeks".into()),
        None => review.push("Never reviewed".into()),
        _ => {}
    }
    let (state, reasons) = if !blocked.is_empty() {
        ("blocked", [blocked, risk, review].concat())
    } else if !risk.is_empty() {
        ("at_risk", [risk, review].concat())
    } else if !review.is_empty() {
        ("needs_review", review)
    } else {
        ("on_track", vec![])
    };
    Health { state: state.into(), reasons }
}

// ---------- skills ----------

#[derive(Clone, Debug)]
pub struct EvidenceInput {
    pub level: Option<i64>,
    pub observed_at: String,
    pub etype: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillReadiness {
    /// Level from the most recent assessed evidence; None = not assessed. Task completion never changes it.
    pub current_level: Option<i64>,
    pub last_assessed_at: Option<String>,
    pub confidence: String,
    pub evidence_count: usize,
    pub recent_evidence_count: usize,
    pub stale: bool,
}

pub const SKILL_STALE_DAYS: i64 = 120;

pub fn skill_readiness(evs: &[EvidenceInput], today: NaiveDate) -> SkillReadiness {
    let mut assessed: Vec<&EvidenceInput> = evs.iter().filter(|e| e.level.is_some()).collect();
    assessed.sort_by(|a, b| a.observed_at.cmp(&b.observed_at));
    let latest = assessed.last();
    let recent: Vec<&EvidenceInput> = evs
        .iter()
        .filter(|e| date(&e.observed_at).is_some_and(|d| (today - d).num_days() <= SKILL_STALE_DAYS))
        .collect();
    let has_formal = recent.iter().any(|e| matches!(e.etype.as_str(), "assessment" | "mock_test" | "code_review" | "certificate"));
    let confidence = match (recent.len(), has_formal) {
        (0, _) => "none",
        (n, true) if n >= 3 => "high",
        (n, _) if n >= 2 => "medium",
        _ => "low",
    };
    let stale = latest.and_then(|e| date(&e.observed_at)).map(|d| (today - d).num_days() > SKILL_STALE_DAYS).unwrap_or(false);
    SkillReadiness {
        current_level: latest.and_then(|e| e.level),
        last_assessed_at: latest.map(|e| e.observed_at.clone()),
        confidence: confidence.into(),
        evidence_count: evs.len(),
        recent_evidence_count: recent.len(),
        stale,
    }
}

// ---------- habits ----------

/// Schedules: {"type":"daily"} | {"type":"weekdays","days":[1..7]} (1 = Monday) | {"type":"weekly","perWeek":n}
pub fn is_due(schedule: &Value, d: NaiveDate) -> bool {
    match schedule.get("type").and_then(|t| t.as_str()).unwrap_or("daily") {
        "weekdays" => {
            let wd = d.weekday().number_from_monday() as i64;
            schedule.get("days").and_then(|x| x.as_array()).is_some_and(|days| days.iter().any(|x| x.as_i64() == Some(wd)))
        }
        // Flexible weekly target: any day may count.
        _ => true,
    }
}

pub fn per_week(schedule: &Value) -> Option<i64> {
    (schedule.get("type").and_then(|t| t.as_str()) == Some("weekly")).then(|| schedule.get("perWeek").and_then(|p| p.as_i64()).unwrap_or(1).max(1))
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct HabitStats {
    /// Consecutive due days (or weeks for weekly targets) completed. Skipped days don't break or extend it.
    pub streak: i64,
    pub streak_unit: String,
    pub done_7: i64,
    pub due_7: i64,
    pub skipped_7: i64,
    /// done / (due − skipped) over the last 7 days.
    pub consistency_7: Option<f64>,
    pub week_done: i64,
    pub week_target: Option<i64>,
    pub due_today: bool,
    pub today_status: Option<String>,
}

fn week_start(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

pub fn habit_stats(schedule: &Value, start: NaiveDate, logs: &HashMap<NaiveDate, String>, today: NaiveDate) -> HabitStats {
    let mut s = HabitStats { due_today: is_due(schedule, today), today_status: logs.get(&today).cloned(), ..Default::default() };
    if let Some(target) = per_week(schedule) {
        s.streak_unit = "weeks".into();
        s.week_target = Some(target);
        let count_week = |ws: NaiveDate| (0..7).filter(|i| logs.get(&(ws + Duration::days(*i))).map(|x| x == "done").unwrap_or(false)).count() as i64;
        s.week_done = count_week(week_start(today));
        let mut ws = week_start(today);
        if s.week_done < target {
            ws -= Duration::days(7); // the current week is still open
        }
        while ws + Duration::days(6) >= start && count_week(ws) >= target {
            s.streak += 1;
            ws -= Duration::days(7);
        }
    } else {
        s.streak_unit = "days".into();
        let mut d = today;
        let mut guard = 0;
        while d >= start && guard < 3660 {
            guard += 1;
            if is_due(schedule, d) {
                match logs.get(&d).map(String::as_str) {
                    Some("done") => s.streak += 1,
                    Some("skipped") => {}
                    // Today without a log does not break the streak yet.
                    None if d == today => {}
                    _ => break,
                }
            }
            d -= Duration::days(1);
        }
    }
    for i in 0..7 {
        let d = today - Duration::days(i);
        if d < start || !is_due(schedule, d) {
            continue;
        }
        s.due_7 += 1;
        match logs.get(&d).map(String::as_str) {
            Some("done") => s.done_7 += 1,
            Some("skipped") => s.skipped_7 += 1,
            _ => {}
        }
    }
    let denom = s.due_7 - s.skipped_7;
    s.consistency_7 = if per_week(schedule).is_some() {
        s.week_target.map(|t| (s.week_done as f64 / t as f64).min(1.0))
    } else {
        (denom > 0).then(|| s.done_7 as f64 / denom as f64)
    };
    s
}

// ---------- money ----------

/// "1,234.50" → 123450 minor units (2 decimals). Rejects more precision than the currency allows.
pub fn parse_money(s: &str) -> Option<i64> {
    let t = s.trim().replace([',', ' ', '฿'], "");
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r.to_string()),
        None => (false, t),
    };
    let (int, frac) = t.split_once('.').unwrap_or((&t, ""));
    if int.is_empty() && frac.is_empty() || frac.len() > 2 || !int.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole: i64 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let cents: i64 = format!("{frac:0<2}").parse().ok()?;
    let v = whole.checked_mul(100)?.checked_add(cents)?;
    Some(if neg { -v } else { v })
}

pub fn format_money(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let a = minor.unsigned_abs();
    format!("{sign}{}.{:02}", a / 100, a % 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d(s: &str) -> NaiveDate {
        date(s).unwrap()
    }

    fn req(id: &str, rtype: &str, sat: &str, w: f64) -> ReqInput {
        ReqInput { id: id.into(), title: id.into(), rtype: rtype.into(), critical: true, weight: w, satisfaction: sat.into(), verification: "unverified".into(), ..Default::default() }
    }

    #[test]
    fn preparation_is_weighted_and_keeps_unknowns_separate() {
        let today = d("2026-10-21");
        let mut math = req("math", "score", "in_progress", 2.0);
        math.target = Some(80.0);
        math.current = Some(60.0);
        let english = req("eng", "score", "in_progress", 1.0); // no value yet → unknown
        let docs = req("docs", "document", "met", 1.0);
        let p = preparation(&[math, english, docs], today);
        assert_eq!(p.unknown_count, 1);
        // (2·0.75 + 1·1) / 3
        assert!((p.progress.unwrap() - 2.5 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn tasks_done_do_not_make_goal_achieved() {
        let reqs = vec![req("r1", "binary", "met", 1.0), req("r2", "binary", "in_progress", 1.0)];
        assert_eq!(task_completion(&[TaskInput { done: true, effort: 30.0 }, TaskInput { done: true, effort: 60.0 }]), Some(1.0));
        let blockers = achievement_blockers(&reqs, "Admitted");
        assert_eq!(blockers.len(), 1);
        assert!(achievement_blockers(&[req("r1", "binary", "met", 1.0)], "Admitted").is_empty());
        assert!(!achievement_blockers(&[req("r1", "binary", "met", 1.0)], " ").is_empty());
    }

    #[test]
    fn roadmap_uses_milestone_weights() {
        let ms = [
            MilestoneInput { weight: 1.0, status: "met".into(), tasks_done: 0.0, tasks_total: 0.0 },
            MilestoneInput { weight: 3.0, status: "in_progress".into(), tasks_done: 1.0, tasks_total: 3.0 },
        ];
        assert!((roadmap_progress(&ms).unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn health_flags_unverified_and_outdated_criteria() {
        let today = d("2026-10-21");
        let mut r = req("r", "external_verification", "in_progress", 1.0);
        let prep = preparation(&[r.clone()], today);
        let h = goal_health(&HealthInput { status: "active", today, start: None, target: None, preparation: &prep, reqs: &[r.clone()], overdue_tasks: 0, last_reviewed_at: Some(today), success_definition: "x" });
        assert_eq!(h.state, "needs_review");
        r.verification = "verified".into();
        r.source_checked_at = Some("2026-01-01".into());
        assert_eq!(verification_state(&r, today), "outdated");
        r.satisfaction = "blocked".into();
        let h = goal_health(&HealthInput { status: "active", today, start: None, target: None, preparation: &prep, reqs: &[r], overdue_tasks: 0, last_reviewed_at: Some(today), success_definition: "x" });
        assert_eq!(h.state, "blocked");
    }

    #[test]
    fn health_at_risk_when_time_outpaces_preparation() {
        let today = d("2026-10-21");
        let mut r = req("r", "score", "in_progress", 1.0);
        r.target = Some(100.0);
        r.current = Some(10.0);
        let prep = preparation(&[r.clone()], today);
        let h = goal_health(&HealthInput { status: "active", today, start: Some(d("2026-01-01")), target: Some(d("2026-12-31")), preparation: &prep, reqs: &[r], overdue_tasks: 0, last_reviewed_at: Some(today), success_definition: "x" });
        assert_eq!(h.state, "at_risk");
    }

    #[test]
    fn skill_level_comes_only_from_assessed_evidence() {
        let today = d("2026-10-21");
        assert_eq!(skill_readiness(&[], today).current_level, None);
        let evs = vec![
            EvidenceInput { level: Some(2), observed_at: "2026-09-01".into(), etype: "mock_test".into() },
            EvidenceInput { level: None, observed_at: "2026-10-01".into(), etype: "artifact".into() },
            EvidenceInput { level: Some(3), observed_at: "2026-10-10".into(), etype: "code_review".into() },
        ];
        let r = skill_readiness(&evs, today);
        assert_eq!(r.current_level, Some(3));
        assert_eq!(r.confidence, "high");
        assert!(!r.stale);
    }

    #[test]
    fn habit_streak_respects_skips_schedule_and_today() {
        let today = d("2026-10-21"); // Wednesday
        let mut logs = HashMap::new();
        logs.insert(d("2026-10-20"), "done".to_string());
        logs.insert(d("2026-10-19"), "skipped".to_string());
        logs.insert(d("2026-10-18"), "done".to_string());
        logs.insert(d("2026-10-16"), "done".to_string());
        let daily = json!({ "type": "daily" });
        let s = habit_stats(&daily, d("2026-10-01"), &logs, today);
        assert_eq!(s.streak, 2, "17th missing breaks it; skip on 19th does not");
        // Mon/Wed/Fri only: 17 (Sat) and 18 (Sun) are not due.
        let mwf = json!({ "type": "weekdays", "days": [1, 3, 5] });
        let s = habit_stats(&mwf, d("2026-10-01"), &logs, today);
        assert_eq!(s.streak, 1, "Mon 19th skipped, Fri 16th done, Wed 14th missing");
        let weekly = json!({ "type": "weekly", "perWeek": 2 });
        let s = habit_stats(&weekly, d("2026-10-01"), &logs, today);
        assert_eq!(s.week_done, 1);
        assert_eq!(s.streak, 1, "week of 12th had 16th+18th");
    }

    #[test]
    fn money_parses_without_floats() {
        assert_eq!(parse_money("1,234.5"), Some(123450));
        assert_eq!(parse_money("฿ 20"), Some(2000));
        assert_eq!(parse_money("0.1"), Some(10));
        assert_eq!(parse_money("1.234"), None);
        assert_eq!(parse_money("abc"), None);
        assert_eq!(format_money(-123450), "-1234.50");
    }
}

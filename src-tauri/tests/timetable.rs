//! Timetable: weekly series, single-occurrence edits, plan vs actual, validation.

use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::application::{goals, timetable};
use serde_json::json;

#[test]
fn weekly_blocks_occurrence_edits_and_plan_vs_actual() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(&dir.path().join("ws")).unwrap();
    let c = &ws.conn;
    let g = goals::create(c, &json!({ "title": "เตรียมสอบ", "domain": "education", "status": "active", "timeBudgetHoursPerWeek": 6 })).unwrap();
    let gid = g["id"].as_str().unwrap();
    // Mon/Wed/Fri 18:00–19:30 maths, for the autumn term.
    let b = timetable::create(c, &json!({ "title": "ติวคณิต", "kind": "study", "recurrence": "weekly", "weekdays": [1, 3, 5], "startTime": "18:00", "endTime": "19:30", "validFrom": "2026-10-01", "validUntil": "2026-12-31", "goalId": gid })).unwrap();
    let bid = b["id"].as_str().unwrap();
    timetable::create(c, &json!({ "title": "สอบย่อย", "kind": "class", "date": "2026-10-20", "startTime": "09:00", "endTime": "10:00" })).unwrap();

    let w = timetable::week(c, "2026-10-21").unwrap();
    assert_eq!(w["weekStart"], "2026-10-19");
    let counts: Vec<usize> = w["days"].as_array().unwrap().iter().map(|d| d["blocks"].as_array().unwrap().len()).collect();
    assert_eq!(counts, vec![1, 1, 1, 0, 1, 0, 0]);
    assert_eq!(w["plannedMinutes"], 4 * 60 + 30 + 60);
    assert_eq!(w["goals"][0]["plannedMinutes"], 270);
    assert_eq!(w["goals"][0]["budgetMinutes"], 360);

    // Move only Wednesday's class to 20:00; the series stays.
    timetable::update_occurrence(c, bid, "2026-10-21", &json!({ "startTime": "20:00", "endTime": "21:00" })).unwrap();
    let w = timetable::week(c, "2026-10-19").unwrap();
    let wed = &w["days"][2]["blocks"];
    assert_eq!(wed.as_array().unwrap().len(), 1);
    assert_eq!(wed[0]["startTime"], "20:00");
    assert_eq!(wed[0]["recurrence"], "once");
    assert_eq!(w["days"][4]["blocks"][0]["startTime"], "18:00");

    // Delete Friday only, then the whole series.
    timetable::delete(c, bid, Some("2026-10-23")).unwrap();
    assert!(timetable::week(c, "2026-10-19").unwrap()["days"][4]["blocks"].as_array().unwrap().is_empty());
    timetable::delete(c, bid, None).unwrap();
    assert!(timetable::week(c, "2026-10-26").unwrap()["days"][0]["blocks"].as_array().unwrap().is_empty());

    // Validation.
    assert!(timetable::create(c, &json!({ "title": "x", "date": "2026-10-20", "startTime": "10:00", "endTime": "09:00" })).is_err());
    assert!(timetable::create(c, &json!({ "title": "x", "recurrence": "weekly", "weekdays": [], "startTime": "10:00", "endTime": "11:00" })).is_err());
    assert!(timetable::create(c, &json!({ "title": "x", "startTime": "10:00", "endTime": "11:00" })).is_err(), "one-off needs a date");
}

#[test]
fn overlapping_blocks_are_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(&dir.path().join("ws")).unwrap();
    let c = &ws.conn;
    timetable::create(c, &json!({ "title": "A", "date": "2026-10-20", "startTime": "09:00", "endTime": "10:30" })).unwrap();
    timetable::create(c, &json!({ "title": "B", "date": "2026-10-20", "startTime": "10:00", "endTime": "11:00" })).unwrap();
    timetable::create(c, &json!({ "title": "C", "date": "2026-10-20", "startTime": "11:00", "endTime": "12:00" })).unwrap();
    let w = timetable::week(c, "2026-10-20").unwrap();
    let flags: Vec<bool> = w["days"][1]["blocks"].as_array().unwrap().iter().map(|b| b["overlaps"].as_bool().unwrap()).collect();
    assert_eq!(flags, vec![true, true, false]);
}

//! Acceptance tests for the Personal Growth extension PRD (§16 user stories, §20 checklist, §18 migration).

use personal_workspace_lib::application::mcp_access::Policy;
use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::application::{finance, goals, habits, reviews, settings, skills, tasks};
use serde_json::json;

fn ws() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(&dir.path().join("ws")).unwrap();
    settings::set_user(&ws.conn, "timezone", &json!("Asia/Bangkok")).unwrap();
    (dir, ws)
}

fn id(v: &serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[test]
fn story_goal_breakdown_links_tasks_to_requirements_and_today() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let g = goals::create(c, &json!({ "title": "Prepare application", "domain": "education", "status": "active", "successDefinition": "Submitted and accepted" })).unwrap();
    let gid = id(&g);
    let mut req_ids = vec![];
    for i in 1..=4 {
        let r = goals::add_requirement(c, &gid, &json!({ "title": format!("R{i}"), "type": "binary", "weight": 1 })).unwrap();
        req_ids.push(id(&r));
    }
    let ms = goals::add_milestone(c, &gid, &json!({ "title": "Foundations", "phase": "Phase 1", "requirementId": req_ids[0] })).unwrap();
    let t = goals::add_milestone_task(c, &id(&ms), &json!({ "title": "ทำโจทย์ 20 ข้อ", "dueDate": tasks::today(c), "estimatedEffortMinutes": 90, "priority": "high" })).unwrap();
    assert_eq!(t.requirement_links.len(), 1, "inherits the milestone's requirement");
    assert_eq!(t.requirement_links[0]["requirementId"], req_ids[0].as_str());
    // Same record appears in Today.
    let today = reviews::today_plan(c).unwrap();
    assert!(today["tasks"].as_array().unwrap().iter().any(|x| x["id"] == t.id.as_str()));

    // All tasks done ≠ achieved.
    tasks::update(c, &t.id, &json!({ "status": "done" })).unwrap();
    let m = goals::metrics(c, &gid).unwrap();
    assert_eq!(m["taskCompletion"]["value"], json!(1.0));
    assert_eq!(m["preparation"]["value"], json!(0.0));
    assert!(goals::update(c, &gid, &json!({ "status": "achieved" })).is_err());
    for r in &req_ids {
        goals::update_requirement(c, r, &json!({ "satisfactionStatus": "met" })).unwrap();
    }
    assert_eq!(goals::update(c, &gid, &json!({ "status": "achieved" })).unwrap()["status"], "achieved");
}

#[test]
fn allocations_are_capped_and_dependencies_cannot_cycle() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let g = goals::create(c, &json!({ "title": "G", "domain": "career" })).unwrap();
    let r1 = id(&goals::add_requirement(c, &id(&g), &json!({ "title": "A" })).unwrap());
    let r2 = id(&goals::add_requirement(c, &id(&g), &json!({ "title": "B" })).unwrap());
    let t = tasks::create(c, &json!({ "title": "T" })).unwrap();
    assert!(tasks::update(c, &t.id, &json!({ "requirementLinks": [{ "requirementId": r1, "allocationPct": 70 }, { "requirementId": r2, "allocationPct": 50 }] })).is_err());
    let ok = tasks::update(c, &t.id, &json!({ "requirementLinks": [{ "requirementId": r1, "allocationPct": 70 }, { "requirementId": r2, "allocationPct": 30 }] })).unwrap();
    assert_eq!(ok.requirement_links.len(), 2);
    let a = tasks::create(c, &json!({ "title": "A" })).unwrap();
    let b = tasks::create(c, &json!({ "title": "B", "dependsOn": [a.id] })).unwrap();
    assert!(tasks::update(c, &a.id, &json!({ "dependsOn": [b.id] })).is_err(), "A→B→A");
    // The task counts once in goal metrics even with two links.
    assert_eq!(goals::goal_task_ids(c, &id(&g)).unwrap().len(), 1);
}

#[test]
fn measured_requirement_below_target_needs_override_reason() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let g = goals::create(c, &json!({ "title": "G", "domain": "education" })).unwrap();
    let r = id(&goals::add_requirement(c, &id(&g), &json!({ "title": "Mock", "type": "score", "targetValue": 80, "currentValue": 60 })).unwrap());
    assert!(goals::update_requirement(c, &r, &json!({ "satisfactionStatus": "met" })).is_err());
    assert!(goals::update_requirement(c, &r, &json!({ "satisfactionStatus": "met", "manualOverrideReason": "Official result received" })).is_ok());
}

#[test]
fn story_skill_growth_comes_from_evidence_not_completion() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let skill = skills::create(c, &json!({ "name": "React", "targetLevel": 4 })).unwrap();
    let sid = id(&skill);
    let g = goals::create(c, &json!({ "title": "Frontend role", "domain": "career" })).unwrap();
    goals::set_goal_skill(c, &id(&g), &sid, Some(4)).unwrap();
    let t = tasks::create(c, &json!({ "title": "Build React dashboard", "skillIds": [sid] })).unwrap();
    tasks::update(c, &t.id, &json!({ "status": "done" })).unwrap();
    assert!(skills::get(c, &sid).unwrap()["readiness"]["currentLevel"].is_null(), "completion alone never sets a level");
    skills::add_evidence(c, &json!({ "skillId": sid, "evidenceType": "code_review", "title": "PR reviewed", "level": 3, "taskId": t.id })).unwrap();
    let s = skills::get(c, &sid).unwrap();
    assert_eq!(s["readiness"]["currentLevel"], 3);
    assert_eq!(s["readiness"]["lastAssessedAt"], tasks::today(c));
    let gap = goals::goal_skills(c, &id(&g)).unwrap();
    assert_eq!(gap[0]["gap"], 1);
    assert!(skills::add_evidence(c, &json!({ "skillId": sid, "evidenceType": "mock_test", "title": "x", "score": 12, "maxScore": 10 })).is_err());
}

#[test]
fn rubric_change_bumps_version_for_new_evidence() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let sid = id(&skills::create(c, &json!({ "name": "English reading" })).unwrap());
    let e1 = skills::add_evidence(c, &json!({ "skillId": sid, "evidenceType": "assessment", "title": "Pre-test", "level": 2 })).unwrap();
    skills::update(c, &sid, &json!({ "rubric": ["0", "1", "2", "3", "4", "5"] })).unwrap();
    let e2 = skills::add_evidence(c, &json!({ "skillId": sid, "evidenceType": "assessment", "title": "Post-test", "level": 3 })).unwrap();
    assert_eq!(e1["rubricVersion"], 1);
    assert_eq!(e2["rubricVersion"], 2);
}

#[test]
fn story_habit_and_budget_without_double_counting() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let h = habits::create(c, &json!({ "title": "Study", "schedule": { "type": "weekly", "perWeek": 5 } })).unwrap();
    let today = tasks::today(c);
    let logged = habits::log(c, &id(&h), &today, Some("done"), None, "").unwrap();
    assert_eq!(logged["stats"]["weekDone"], 1);
    habits::log(c, &id(&h), &today, Some("done"), None, "").unwrap();
    assert_eq!(habits::get(c, &id(&h)).unwrap()["stats"]["weekDone"], 1, "one log per day");
    habits::log(c, &id(&h), &today, Some("skipped"), None, "").unwrap();
    assert_eq!(habits::get(c, &id(&h)).unwrap()["stats"]["todayStatus"], "skipped");
    assert!(habits::log(c, &id(&h), "2999-01-01", Some("done"), None, "").is_err());

    let cash = id(&finance::create_account(c, &json!({ "name": "Cash", "openingBalance": "1,000" })).unwrap());
    let bank = id(&finance::create_account(c, &json!({ "name": "Bank", "type": "bank" })).unwrap());
    let g = goals::create(c, &json!({ "title": "Course", "domain": "education" })).unwrap();
    finance::add_transaction(c, &json!({ "type": "income", "accountId": bank, "amount": "5000", "categoryId": "cat-side" })).unwrap();
    finance::add_transaction(c, &json!({ "type": "expense", "accountId": bank, "amount": "1200.50", "categoryId": "cat-education", "linkedGoalId": id(&g) })).unwrap();
    let tr = finance::add_transaction(c, &json!({ "type": "transfer", "fromAccountId": bank, "toAccountId": cash, "amount": "300" })).unwrap();
    finance::set_budget(c, "cat-education", &json!({ "limit": "1000" })).unwrap();
    let month = &today[..7];
    let m = finance::month_summary(c, month).unwrap();
    let thb = &m["currencies"][0];
    assert_eq!(thb["incomeMinor"], 500000);
    assert_eq!(thb["expenseMinor"], 120050, "transfer is not an expense");
    assert_eq!(thb["totalBalanceMinor"], 100000 + 500000 - 120050);
    assert_eq!(thb["budgets"][0]["over"], true);
    assert_eq!(finance::goal_finance(c, &id(&g)).unwrap()["spent"][0]["spentMinor"], 120050);

    // Savings allocations earmark money; they cannot exceed unallocated cash.
    let sg = finance::create_savings_goal(c, &json!({ "title": "Laptop", "target": "4000" })).unwrap();
    assert!(finance::allocate(c, &id(&sg), &json!({ "amount": "5000" })).is_err());
    finance::allocate(c, &id(&sg), &json!({ "amount": "2000" })).unwrap();
    let s = finance::savings_goals(c, None).unwrap();
    assert_eq!(s[0]["progress"], 0.5);
    let thb = finance::month_summary(c, month).unwrap()["currencies"][0].clone();
    assert_eq!(thb["availableCashMinor"], thb["totalBalanceMinor"].as_i64().unwrap() - 200000);

    // Deleting one half of a transfer removes both; balances stay consistent; the change is audited.
    finance::delete_transaction(c, &id(&tr)).unwrap();
    let accts = finance::accounts(c, false).unwrap();
    assert_eq!(accts.iter().find(|a| a["id"] == cash.as_str()).unwrap()["balanceMinor"], 100000);
    assert!(finance::audit_trail(c, 10).unwrap().iter().any(|a| a["action"] == "delete"));
}

#[test]
fn mcp_finance_needs_explicit_grant() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    settings::set_user(c, "mcp", &json!({ "enabled": true, "scope": "all", "allowedIds": [] })).unwrap();
    let p = Policy::load(c).unwrap();
    assert!(p.domain_allowed("goals") && p.domain_allowed("habits"));
    assert!(!p.domain_allowed("finance"), "scope=all still excludes finance");
    settings::set_user(c, "mcp", &json!({ "enabled": true, "scope": "selected", "allowedIds": ["domain:finance"] })).unwrap();
    let p = Policy::load(c).unwrap();
    assert!(p.domain_allowed("finance") && !p.domain_allowed("goals"));
}

#[test]
fn ai_drafts_change_nothing_until_applied() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let gid = id(&goals::create(c, &json!({ "title": "G", "domain": "education" })).unwrap());
    let draft = goals::create_draft(c, &gid, &json!({ "requirements": [
        { "title": "English score", "type": "score", "targetValue": 70, "milestones": [{ "title": "Mock 1", "tasks": [{ "title": "Timed reading", "estimatedMinutes": 45 }, { "title": "Vocabulary" }] }] }
    ]}), "mcp:test").unwrap();
    assert!(goals::requirements(c, &gid).unwrap().is_empty());
    let did = draft["draftId"].as_str().unwrap();
    let r = goals::resolve_draft(c, did, true).unwrap();
    assert_eq!(r["created"], json!({ "requirements": 1, "milestones": 1, "tasks": 2 }));
    assert_eq!(goals::goal_task_ids(c, &gid).unwrap().len(), 2);
    assert!(goals::resolve_draft(c, did, true).is_err(), "applied once");
    assert!(goals::create_draft(c, &gid, &json!({ "requirements": [{ "title": "" }] }), "x").is_err());
}

#[test]
fn weekly_review_summarises_local_data() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let t = tasks::create(c, &json!({ "title": "Done this week", "dueDate": tasks::today(c) })).unwrap();
    tasks::update(c, &t.id, &json!({ "status": "done" })).unwrap();
    let sid = id(&skills::create(c, &json!({ "name": "Math" })).unwrap());
    skills::add_evidence(c, &json!({ "skillId": sid, "evidenceType": "mock_test", "title": "Mock A", "level": 2 })).unwrap();
    let week = reviews::current_week(c);
    let r = reviews::save(c, &week, &json!({ "reflection": "ok", "nextCommitments": "2 mocks" })).unwrap();
    assert_eq!(r["draft"]["completedTasks"].as_array().unwrap().len(), 1);
    assert_eq!(r["draft"]["evidence"].as_array().unwrap().len(), 1);
    assert_eq!(r["saved"]["nextCommitments"], "2 mocks");
}

#[test]
fn example_goal_is_marked_illustrative_and_unverified() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let g = goals::create_example(c).unwrap();
    assert_eq!(g["isExample"], true);
    let m = goals::metrics(c, &id(&g)).unwrap();
    assert_eq!(m["preparation"]["total"], 4);
    assert!(m["preparation"]["unverified"].as_i64().unwrap() >= 1);
    assert_ne!(m["health"]["state"], "on_track");
}

#[test]
fn v1_workspace_migrates_without_fabricating_growth_data() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("old");
    std::fs::create_dir_all(&root).unwrap();
    {
        let conn = rusqlite::Connection::open(root.join("workspace.sqlite")).unwrap();
        conn.execute_batch(include_str!("../migrations/0001_init.sql")).unwrap();
        conn.execute_batch(
            "PRAGMA user_version=1;
             INSERT INTO workspaces VALUES ('w1','Old','2026-01-01T00:00:00.000Z');
             INSERT INTO sections VALUES ('s1','Knowledge',NULL,'knowledge',0,'2026-01-01T00:00:00.000Z');
             INSERT INTO tasks(id,title,created_at,updated_at) VALUES ('t1','Legacy task','2026-01-01T00:00:00.000Z','2026-01-01T00:00:00.000Z');",
        )
        .unwrap();
    }
    let ws = Workspace::open(&root).unwrap();
    let t = tasks::get(&ws.conn, "t1").unwrap();
    assert_eq!(t.title, "Legacy task");
    assert!(t.requirement_links.is_empty() && t.estimated_effort_minutes.is_none());
    assert!(goals::list(&ws.conn, None, true).unwrap().is_empty());
    let backups: Vec<_> = std::fs::read_dir(root.join("backups")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert!(backups.iter().any(|b| b.starts_with("pre-migration-v1")), "{backups:?}");
}

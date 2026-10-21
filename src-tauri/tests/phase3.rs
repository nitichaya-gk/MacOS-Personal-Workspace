//! Acceptance tests for PRD phase 3: formulas/relations/rollups (AC-15), forms (AC-16),
//! automations (AC-17), MCP writes (AC-18), versions and comments (FR-15).

use personal_workspace_lib::application::mcp_access::Policy;
use personal_workspace_lib::application::pages::{self, CreatePage};
use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::application::{automations, comments, databases, forms, settings, tasks, versions};
use personal_workspace_lib::integrations::mcp;
use serde_json::{json, Map, Value};

fn ws() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(&dir.path().join("ws")).unwrap();
    (dir, ws)
}

fn db(ws: &Workspace, title: &str) -> String {
    pages::create(&ws.conn, CreatePage { kind: "database".into(), title: title.into(), ..Default::default() }).unwrap().id
}

fn prop(ws: &Workspace, db_id: &str, name: &str, t: &str, config: Value) -> String {
    let p = databases::add_property(&ws.conn, db_id, name, t).unwrap();
    if !config.is_null() {
        databases::update_property(&ws.conn, &p.id, &json!({ "config": config })).unwrap();
    }
    p.id
}

fn rec(ws: &Workspace, db_id: &str, title: &str, values: Value) -> String {
    let values: Map<String, Value> = values.as_object().cloned().unwrap_or_default();
    databases::create_record(&ws.conn, db_id, databases::CreateRecord { title: title.into(), values, ..Default::default() }).unwrap().id
}

#[test]
fn ac15_formula_relation_rollup_and_cycles() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let items = db(&ws, "รายการ");
    let price = prop(&ws, &items, "ราคา", "number", Value::Null);
    let qty = prop(&ws, &items, "จำนวน", "number", Value::Null);
    let total = prop(&ws, &items, "รวม", "formula", json!({ "expression": "prop(\"ราคา\") * prop(\"จำนวน\")" }));
    let a = rec(&ws, &items, "A", json!({ price.clone(): 10, qty.clone(): 3 }));
    let b = rec(&ws, &items, "B", json!({ price.clone(): 2.5, qty.clone(): 4 }));

    let orders = db(&ws, "คำสั่งซื้อ");
    let rel = prop(&ws, &orders, "Items", "relation", json!({ "databaseId": items }));
    let sum = prop(&ws, &orders, "Total", "rollup", json!({ "relationPropertyId": rel, "targetPropertyId": total, "function": "sum" }));
    let count = prop(&ws, &orders, "Count", "rollup", json!({ "relationPropertyId": rel, "function": "count" }));
    let label = prop(&ws, &orders, "Label", "formula", json!({ "expression": "concat(prop(\"Name\"), \": \", prop(\"Total\"), \" บาท\")" }));
    let o = rec(&ws, &orders, "O-1", json!({ rel.clone(): [a, b] }));

    let r = databases::get_record(c, &o).unwrap();
    assert_eq!(r.values[&sum], json!(40.0));
    assert_eq!(r.values[&count], json!(2.0));
    assert_eq!(r.values[&label], json!("O-1: 40 บาท"));
    assert_eq!(r.relation_titles.len(), 2);

    // Filtering and sorting work on computed values.
    let q = databases::query(c, &orders, &databases::QuerySpec { filter: Some(json!({ "op": "and", "conditions": [{ "propertyId": sum, "operator": "gt", "value": 39 }] })), ..Default::default() }).unwrap();
    assert_eq!(q.total, 1);

    // A relation can only point at records of its target database.
    assert!(databases::set_value(c, &o, &rel, &json!([o])).is_err());

    // Cycles are refused with a readable path; syntax errors too.
    let f1 = prop(&ws, &items, "F1", "formula", json!({ "expression": "1" }));
    let _f2 = prop(&ws, &items, "F2", "formula", json!({ "expression": "prop(\"F1\") + 1" }));
    let err = databases::update_property(c, &f1, &json!({ "config": { "expression": "prop(\"F2\") * 2" } })).unwrap_err().to_string();
    assert!(err.contains("Circular formula") && err.contains("F1") && err.contains("F2"), "{err}");
    assert!(databases::update_property(c, &f1, &json!({ "config": { "expression": "prop(\"F2\" *" } })).unwrap_err().to_string().contains("Formula error"));
    // Runtime errors stay inside the cell instead of breaking the database.
    let bad = prop(&ws, &items, "Bad", "formula", json!({ "expression": "prop(\"ราคา\") / 0" }));
    let r = databases::get_record(c, &rec(&ws, &items, "C", json!({ price.clone(): 1 }))).unwrap();
    assert!(r.values[&bad]["error"].as_str().unwrap().contains("division"));
    assert!(databases::to_csv(c, &orders).unwrap().contains("A; B"));
}

#[test]
fn ac16_forms_create_one_record_or_explain() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let d = db(&ws, "สมัครกิจกรรม");
    let email = prop(&ws, &d, "Email", "email", Value::Null);
    let f = forms::create(c, &d, "สมัคร").unwrap();
    let fid = f["id"].as_str().unwrap();
    let mut cfg = f["config"].clone();
    for field in cfg["fields"].as_array_mut().unwrap() {
        if field["propertyId"] == email.as_str() {
            field["required"] = json!(true);
        }
    }
    forms::update(c, fid, &json!({ "config": cfg })).unwrap();
    let title = databases::get(c, &d).unwrap().properties.iter().find(|p| p.ptype == "title").unwrap().id.clone();

    let bad = forms::submit(c, fid, &json!({ title.clone(): "", email.clone(): "not-an-email" }).as_object().unwrap().clone()).unwrap();
    assert_eq!(bad["ok"], false);
    assert!(bad["errors"][&title].as_str().unwrap().contains("required"));
    assert!(bad["errors"][&email].is_string());
    assert_eq!(databases::query(c, &d, &Default::default()).unwrap().total, 0);

    let ok = forms::submit(c, fid, &json!({ title.clone(): "สมชาย", email.clone(): "a@b.co" }).as_object().unwrap().clone()).unwrap();
    assert_eq!(ok["ok"], true);
    assert_eq!(databases::query(c, &d, &Default::default()).unwrap().total, 1);
}

#[test]
fn ac17_automations_run_once_log_and_never_loop() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let d = db(&ws, "Leads");
    let info = databases::get(c, &d).unwrap();
    let status = info.properties.iter().find(|p| p.ptype == "status").unwrap().clone();
    let todo = status.config["options"][0]["id"].as_str().unwrap().to_string();
    let done = status.config["options"][2]["id"].as_str().unwrap().to_string();
    let note = prop(&ws, &d, "Note", "text", Value::Null);

    // On create: set status To do and create a follow-up task.
    automations::create(c, "triage", &json!({
        "trigger": { "type": "record_created", "databaseId": d },
        "actions": [{ "type": "set_property", "propertyId": status.id, "value": todo }, { "type": "create_task", "title": "Follow up: {{title}}", "dueInDays": 1 }]
    })).unwrap();
    // Condition: only when status is Done → create a task.
    automations::create(c, "done", &json!({
        "trigger": { "type": "record_updated", "databaseId": d, "propertyId": status.id },
        "condition": { "filter": { "op": "and", "conditions": [{ "propertyId": status.id, "operator": "is", "value": done }] } },
        "actions": [{ "type": "create_task", "title": "Celebrate {{title}}" }]
    })).unwrap();
    // A rule that would loop forever: on any Note change, change Note.
    automations::create(c, "loop", &json!({
        "trigger": { "type": "record_updated", "databaseId": d, "propertyId": note },
        "actions": [{ "type": "set_property", "propertyId": note, "value": "again" }]
    })).unwrap();

    let r = rec(&ws, &d, "ACME", json!({}));
    let rv = databases::get_record(c, &r).unwrap();
    assert_eq!(rv.values[&status.id], json!(todo));
    let open = tasks::list(c, "all", None, None, 50).unwrap();
    assert_eq!(open.iter().filter(|t| t.title == "Follow up: ACME").count(), 1);

    databases::set_value(c, &r, &status.id, &json!(done)).unwrap();
    assert_eq!(tasks::list(c, "all", None, None, 50).unwrap().iter().filter(|t| t.title == "Celebrate ACME").count(), 1);

    databases::set_value(c, &r, &note, &json!("hi")).unwrap();
    let runs = automations::runs(c, None, 50).unwrap();
    assert!(runs.iter().any(|x| x["status"] == "skipped" && x["detail"].as_str().unwrap().contains("Loop")), "{runs:?}");
    assert!(runs.iter().filter(|x| x["automationName"] == "loop" && x["status"] == "ok").count() <= 1);

    // Scheduled rules run at most once per day.
    settings::set_user(c, "timezone", &json!("UTC")).unwrap();
    automations::create(c, "daily", &json!({ "trigger": { "type": "schedule", "time": "00:00" }, "actions": [{ "type": "notify", "title": "Morning" }] })).unwrap();
    automations::tick_schedules(c).unwrap();
    automations::tick_schedules(c).unwrap();
    assert_eq!(automations::take_outbox(c).unwrap().len(), 1);

    // A failing action rolls back the whole run and is logged as an error.
    automations::create(c, "broken", &json!({
        "trigger": { "type": "record_created", "databaseId": d },
        "actions": [{ "type": "create_task", "title": "should roll back" }, { "type": "create_page_from_template", "templateId": "missing" }]
    })).unwrap();
    rec(&ws, &d, "Second", json!({}));
    assert!(tasks::list(c, "all", None, None, 100).unwrap().iter().all(|t| t.title != "should roll back"));
    let failed = automations::runs(c, None, 100).unwrap().into_iter().find(|x| x["automationName"] == "broken").unwrap();
    assert_eq!(failed["status"], "error");
}

#[test]
fn ac18_mcp_writes_use_services_need_grants_and_are_revisioned() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let page = pages::create(c, CreatePage { kind: "page".into(), title: "Notes".into(), ..Default::default() }).unwrap();
    settings::set_user(c, "mcp", &json!({ "enabled": true, "scope": "all", "allowedIds": [] })).unwrap();
    let p = Policy::load(c).unwrap();
    let denied = mcp::run_tool(&ws, &p, "update_page", &json!({ "pageId": page.id, "appendMarkdown": "x" }), "test");
    assert!(denied.unwrap_err().to_string().contains("not open for MCP writes"));

    settings::set_user(c, "mcp", &json!({ "enabled": true, "scope": "all", "allowedIds": [], "writeIds": [page.section_id.clone().unwrap(), "tasks:unassigned"] })).unwrap();
    let p = Policy::load(c).unwrap();
    let dry = mcp::run_tool(&ws, &p, "update_page", &json!({ "pageId": page.id, "appendMarkdown": "- [ ] งานใหม่", "dryRun": true }), "test").unwrap();
    assert_eq!(dry["dryRun"], true);
    assert!(pages::load_blocks(c, &page.id).unwrap().is_empty());
    mcp::run_tool(&ws, &p, "update_page", &json!({ "pageId": page.id, "appendMarkdown": "## หัวข้อ\n- [ ] งานใหม่" }), "test").unwrap();
    assert_eq!(pages::load_blocks(c, &page.id).unwrap().len(), 2);
    assert!(versions::list(c, &page.id).unwrap().iter().any(|v| v["source"] == "mcp:test"));
    let t = mcp::run_tool(&ws, &p, "create_task", &json!({ "title": "From Claude" }), "test").unwrap();
    assert!(tasks::get(c, t["taskId"].as_str().unwrap()).is_ok());
    // Finance is never writable, goals need their own grant.
    assert!(p.require_write_domain("finance").is_err());
    assert!(mcp::run_tool(&ws, &p, "log_focus_session", &json!({ "minutes": 30 }), "test").is_err());
}

#[test]
fn versions_restore_and_comments() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let page = pages::create(c, CreatePage { kind: "page".into(), title: "Draft".into(), ..Default::default() }).unwrap();
    let v1 = json!([{ "id": "a", "type": "paragraph", "props": {}, "content": [{ "type": "text", "text": "ฉบับแรก", "styles": {} }], "children": [] }]);
    pages::save_blocks(c, &page.id, &v1).unwrap();
    versions::snapshot(c, &page.id, "manual").unwrap();
    pages::save_blocks(c, &page.id, &json!([])).unwrap();
    let list = versions::list(c, &page.id).unwrap();
    let id = list.iter().find(|v| v["source"] == "manual").unwrap()["id"].as_str().unwrap().to_string();
    assert!(versions::get(c, &id).unwrap()["markdown"].as_str().unwrap().contains("ฉบับแรก"));
    versions::restore(c, &id).unwrap();
    assert_eq!(pages::load_blocks(c, &page.id).unwrap(), v1.as_array().unwrap().clone());
    assert!(versions::list(c, &page.id).unwrap().iter().any(|v| v["source"] == "before_restore"));

    let cm = comments::add(c, &page.id, Some("a"), "ฉบับแรก", "ตรวจคำนี้").unwrap();
    comments::update(c, cm["id"].as_str().unwrap(), None, Some(true)).unwrap();
    let all = comments::list(c, &page.id).unwrap();
    assert!(all[0]["resolvedAt"].is_string());
    assert!(comments::add(c, &page.id, None, "", "  ").is_err());
}

#[test]
fn v2_workspace_migrates_to_v3_keeping_views() {
    let (_d, ws) = ws();
    let d = db(&ws, "Old");
    databases::add_view(&ws.conn, &d, "Board", "board").unwrap();
    databases::add_view(&ws.conn, &d, "Gantt", "timeline").unwrap();
    databases::add_view(&ws.conn, &d, "Chart", "chart").unwrap();
    assert_eq!(databases::views(&ws.conn, &d).unwrap().len(), 4);
}

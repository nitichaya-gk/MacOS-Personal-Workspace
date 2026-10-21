//! Integration tests mapped to PRD §16 acceptance criteria (phases 1–2), run against real SQLite files.

use personal_workspace_lib::application::mcp_access::Policy;
use personal_workspace_lib::application::pages::{self, CreatePage};
use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::application::{canvases, databases, export, settings, tasks, templates};
use personal_workspace_lib::infrastructure::{backup, search};
use serde_json::json;

fn ws() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(&dir.path().join("ws")).unwrap();
    (dir, ws)
}

fn page(ws: &Workspace, title: &str, parent: Option<&str>) -> pages::PageMeta {
    pages::create(&ws.conn, CreatePage { kind: "page".into(), title: title.into(), parent_id: parent.map(String::from), ..Default::default() }).unwrap()
}

fn mention(id: &str) -> serde_json::Value {
    json!([{ "id": "b1", "type": "paragraph", "content": [
        { "type": "text", "text": "ดูเพิ่มเติมที่ ", "styles": {} },
        { "type": "pageMention", "props": { "pageId": id } }
    ], "children": [] }])
}

#[test]
fn ac01_tree_moves_keep_links_and_reject_cycles() {
    let (_d, ws) = ws();
    let a = page(&ws, "A", None);
    let b = page(&ws, "B", Some(&a.id));
    let c = page(&ws, "C", Some(&b.id));
    let other = page(&ws, "Other", None);
    pages::save_blocks(&ws.conn, &other.id, &mention(&c.id)).unwrap();

    pages::update(&ws.conn, &c.id, &json!({ "title": "C renamed" })).unwrap();
    pages::move_page(&ws.conn, &c.id, Some(&a.id), None, Some(0)).unwrap();
    let back = pages::backlinks(&ws.conn, &c.id).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].source_id, other.id);
    assert!(export::page_markdown(&ws.conn, &other.id, None).unwrap().text.contains("C renamed"));

    // A cannot move under its own descendant.
    assert!(pages::move_page(&ws.conn, &a.id, Some(&b.id), None, None).is_err());
    assert!(pages::move_page(&ws.conn, &a.id, Some(&a.id), None, None).is_err());
}

#[test]
fn ac02_thai_blocks_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    let blocks = json!([
        { "id": "h", "type": "heading", "props": { "level": 2 }, "content": [{ "type": "text", "text": "บันทึกการประชุม", "styles": { "bold": true } }], "children": [] },
        { "id": "l", "type": "bulletListItem", "props": {}, "content": [{ "type": "text", "text": "หัวข้อหลัก", "styles": {} }], "children": [
            { "id": "l2", "type": "checkListItem", "props": { "checked": true }, "content": [{ "type": "text", "text": "งานย่อย", "styles": {} }], "children": [] }
        ] }
    ]);
    let id = {
        let ws = Workspace::open(&root).unwrap();
        let p = page(&ws, "หน้าแรก", None);
        pages::save_blocks(&ws.conn, &p.id, &blocks).unwrap();
        p.id
    };
    let ws = Workspace::open(&root).unwrap();
    let loaded = pages::load_blocks(&ws.conn, &id).unwrap();
    assert_eq!(serde_json::Value::Array(loaded), blocks);
    assert_eq!(pages::get_meta(&ws.conn, &id).unwrap().title, "หน้าแรก");
}

#[test]
fn ac03_database_filter_sort_persist() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    let (db_id, view_id) = {
        let ws = Workspace::open(&root).unwrap();
        let db = pages::create(&ws.conn, CreatePage { kind: "database".into(), title: "งาน".into(), ..Default::default() }).unwrap();
        let info = databases::get(&ws.conn, &db.id).unwrap();
        let num = databases::add_property(&ws.conn, &db.id, "คะแนน", "number").unwrap();
        for (t, n) in [("ก", 3), ("ข", 9), ("ค", 5)] {
            let mut values = serde_json::Map::new();
            values.insert(num.id.clone(), json!(n));
            databases::create_record(&ws.conn, &db.id, databases::CreateRecord { title: t.into(), values, ..Default::default() }).unwrap();
        }
        let view = &info.views[0];
        databases::update_view(&ws.conn, &view.id, &json!({ "config": {
            "filter": { "op": "and", "conditions": [{ "propertyId": num.id, "operator": "gte", "value": 4 }] },
            "sorts": [{ "propertyId": num.id, "direction": "desc" }]
        }})).unwrap();
        (db.id, view.id.clone())
    };
    let ws = Workspace::open(&root).unwrap();
    let res = databases::query(&ws.conn, &db_id, &databases::QuerySpec { view_id: Some(view_id), ..Default::default() }).unwrap();
    let titles: Vec<_> = res.records.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, vec!["ข", "ค"]);
}

#[test]
fn ac04_today_and_overdue_follow_time_zone() {
    let (_d, ws) = ws();
    settings::set_user(&ws.conn, "timezone", &json!("Asia/Bangkok")).unwrap();
    let today = tasks::today(&ws.conn);
    let yesterday = (chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d").unwrap() - chrono::Duration::days(1)).to_string();
    tasks::create(&ws.conn, &json!({ "title": "วันนี้", "dueDate": today })).unwrap();
    tasks::create(&ws.conn, &json!({ "title": "เมื่อวาน", "dueDate": yesterday })).unwrap();
    let t = tasks::list(&ws.conn, "today", None, None, 50).unwrap();
    let o = tasks::list(&ws.conn, "overdue", None, None, 50).unwrap();
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].title, "วันนี้");
    assert_eq!(o[0].title, "เมื่อวาน");
}

fn pending(ws: &Workspace, task: &str) -> Vec<String> {
    ws.conn
        .prepare("SELECT fire_at FROM reminders WHERE task_id=?1 AND status='pending'")
        .unwrap()
        .query_map([task], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

#[test]
fn ac06_rescheduling_or_completing_never_duplicates_reminders() {
    let (_d, ws) = ws();
    let tz = settings::user_tz(&ws.conn);
    let at = |mins: i64| (chrono::Utc::now() + chrono::Duration::minutes(mins)).with_timezone(&tz).format("%Y-%m-%dT%H:%M").to_string();
    let t = tasks::create(&ws.conn, &json!({ "title": "โทรหาลูกค้า", "remindAtLocal": at(30) })).unwrap();
    assert_eq!(pending(&ws, &t.id).len(), 1);
    tasks::update(&ws.conn, &t.id, &json!({ "remindAtLocal": at(90) })).unwrap();
    let p = pending(&ws, &t.id);
    assert_eq!(p.len(), 1);
    assert_eq!(Some(p[0].clone()), tasks::get(&ws.conn, &t.id).unwrap().remind_at);
    tasks::update(&ws.conn, &t.id, &json!({ "status": "done" })).unwrap();
    assert!(pending(&ws, &t.id).is_empty());
    tasks::update(&ws.conn, &t.id, &json!({ "status": "todo" })).unwrap();
    tasks::delete(&ws.conn, &t.id).unwrap();
    assert!(pending(&ws, &t.id).is_empty());
}

#[test]
fn missed_reminders_are_batched_not_fired() {
    let (_d, ws) = ws();
    let t = tasks::create(&ws.conn, &json!({ "title": "late" })).unwrap();
    ws.conn
        .execute(
            "INSERT INTO reminders(id, task_id, fire_at, status, created_at) VALUES ('r1', ?1, '2020-01-01T00:00:00.000Z', 'pending', '2020-01-01T00:00:00.000Z')",
            [&t.id],
        )
        .unwrap();
    let batch = tasks::take_due_reminders(&ws.conn, 120).unwrap();
    assert!(batch.fire.is_empty());
    assert_eq!(batch.missed.len(), 1);
    assert!(tasks::take_due_reminders(&ws.conn, 120).unwrap().missed.is_empty(), "claimed only once");
}

#[test]
fn ac07_search_finds_thai_substrings_offline() {
    let (_d, ws) = ws();
    let p = page(&ws, "แผนงาน", None);
    pages::save_blocks(&ws.conn, &p.id, &json!([{ "id": "x", "type": "paragraph", "content": [{ "type": "text", "text": "ประชุมทีมออกแบบประจำสัปดาห์", "styles": {} }], "children": [] }])).unwrap();
    let hits = search::search(&ws.conn, "ออกแบบ", &Default::default()).unwrap();
    assert!(hits.iter().any(|h| h.entity_id == p.id));
    // Short (<3 char) queries fall back to LIKE.
    assert!(search::search(&ws.conn, "ทีม", &Default::default()).unwrap().iter().any(|h| h.entity_id == p.id));
    pages::trash(&ws.conn, &p.id).unwrap();
    assert!(!search::search(&ws.conn, "ออกแบบ", &Default::default()).unwrap().iter().any(|h| h.entity_id == p.id));
}

#[test]
fn ac08_backup_restore_keeps_ids_links_and_assets() {
    let (dir, ws) = ws();
    let a = page(&ws, "ต้นทาง", None);
    let b = page(&ws, "ปลายทาง", None);
    pages::save_blocks(&ws.conn, &a.id, &mention(&b.id)).unwrap();
    let asset = personal_workspace_lib::application::assets::upload(&ws, "รูป.png", "image/png", Some(&a.id), b"\x89PNG fake").unwrap();
    let info = backup::create(&ws.conn, &ws.root, &ws.manifest().unwrap(), "").unwrap();
    let dest = dir.path().join("restored");
    let report = backup::restore(std::path::Path::new(info["path"].as_str().unwrap()), &dest).unwrap();
    assert_eq!(report["integrity"]["ok"], json!(true), "{report}");
    let restored = Workspace::open(&dest).unwrap();
    assert_eq!(restored.id, ws.id);
    assert_eq!(pages::backlinks(&restored.conn, &b.id).unwrap()[0].source_id, a.id);
    assert!(dest.join("assets").join(&asset.id).exists());
    // Restoring over a non-empty folder is refused.
    assert!(backup::restore(std::path::Path::new(info["path"].as_str().unwrap()), &dest).is_err());

    // Zip export round-trips too.
    let zip = dir.path().join("export.zip");
    backup::export_zip(&ws.conn, &ws.root, &ws.manifest().unwrap(), &zip).unwrap();
    let report = backup::restore(&zip, &dir.path().join("from-zip")).unwrap();
    assert_eq!(report["integrity"]["ok"], json!(true));
}

#[test]
fn ac09_csv_keeps_thai_columns_and_markdown_is_plain() {
    let (_d, ws) = ws();
    let db = databases::import_csv(&ws.conn, "\u{feff}ชื่องาน,สถานะ,วันที่,จำนวน\nออกแบบ,\"กำลังทำ, รอ\",2026-10-21,3\n", "นำเข้า", None, None).unwrap();
    let csv = databases::to_csv(&ws.conn, &db.id).unwrap();
    assert!(csv.starts_with("\u{feff}ชื่องาน,สถานะ,วันที่,จำนวน"), "{csv}");
    assert!(csv.contains("\"กำลังทำ, รอ\""));
    let info = databases::get(&ws.conn, &db.id).unwrap();
    let types: Vec<_> = info.properties.iter().map(|p| p.ptype.as_str()).collect();
    assert_eq!(types, vec!["title", "text", "date", "number"]);

    let p = page(&ws, "เอกสาร", None);
    pages::save_blocks(&ws.conn, &p.id, &json!([
        { "id": "1", "type": "heading", "props": { "level": 1 }, "content": [{ "type": "text", "text": "หัวข้อ", "styles": {} }], "children": [] },
        { "id": "2", "type": "checkListItem", "props": { "checked": true }, "content": [{ "type": "text", "text": "เสร็จ", "styles": {} }], "children": [] },
        { "id": "3", "type": "mermaid", "props": { "source": "graph TD; A-->B" }, "children": [] }
    ])).unwrap();
    let md = export::page_markdown(&ws.conn, &p.id, None).unwrap().text;
    assert!(md.contains("# หัวข้อ") && md.contains("- [x] เสร็จ") && md.contains("```mermaid"), "{md}");
}

#[test]
fn ac10_canvas_objects_stay_editable() {
    let (_d, ws) = ws();
    let c = pages::create(&ws.conn, CreatePage { kind: "canvas".into(), title: "ระบบ".into(), canvas_kind: Some("diagram".into()), ..Default::default() }).unwrap();
    let target = page(&ws, "Spec", None);
    let elements = vec![
        json!({ "id": "r1", "type": "rectangle", "x": 0, "y": 0, "width": 100, "height": 50, "link": format!("workspace://page/{}", target.id) }),
        json!({ "id": "r2", "type": "rectangle", "x": 300, "y": 0, "width": 100, "height": 50 }),
        json!({ "id": "a1", "type": "arrow", "x": 100, "y": 25, "startBinding": { "elementId": "r1" }, "endBinding": { "elementId": "r2" } }),
        json!({ "id": "gone", "type": "rectangle", "isDeleted": true }),
    ];
    canvases::save(&ws.conn, &c.id, &elements, &json!({ "viewBackgroundColor": "#fff" })).unwrap();
    let loaded = canvases::load(&ws, &c.id).unwrap();
    assert_eq!(loaded["elements"].as_array().unwrap().len(), 3);
    let summary = canvases::summary(&ws.conn, &c.id).unwrap();
    assert_eq!(summary["connections"][0]["from"], "r1");
    assert_eq!(pages::backlinks(&ws.conn, &target.id).unwrap()[0].source_type, "canvas");
}

#[test]
fn ac13_template_instances_never_touch_the_original() {
    let (_d, ws) = ws();
    let tpl = templates::list(&ws.conn).unwrap().into_iter().find(|t| t.title == "Meeting Notes").unwrap();
    let before = pages::load_blocks(&ws.conn, &tpl.id).unwrap();
    let copy = templates::instantiate(&ws.conn, &tpl.id, None, None, Some("ประชุม 21 ต.ค.")).unwrap();
    assert_ne!(copy.id, tpl.id);
    assert!(!copy.is_template);
    pages::save_blocks(&ws.conn, &copy.id, &json!([])).unwrap();
    pages::update(&ws.conn, &copy.id, &json!({ "title": "changed" })).unwrap();
    assert_eq!(pages::load_blocks(&ws.conn, &tpl.id).unwrap(), before);
    assert_eq!(pages::get_meta(&ws.conn, &tpl.id).unwrap().title, "Meeting Notes");
}

#[test]
fn record_templates_copy_values() {
    let (_d, ws) = ws();
    let db = pages::create(&ws.conn, CreatePage { kind: "database".into(), title: "Bugs".into(), ..Default::default() }).unwrap();
    let info = databases::get(&ws.conn, &db.id).unwrap();
    let status = info.properties.iter().find(|p| p.ptype == "status").unwrap();
    let done = status.config["options"][2]["id"].as_str().unwrap().to_string();
    let mut values = serde_json::Map::new();
    values.insert(status.id.clone(), json!(done));
    let tpl = databases::create_record(&ws.conn, &db.id, databases::CreateRecord { title: "Bug report".into(), values, is_template: true, ..Default::default() }).unwrap();
    let rec = databases::create_record(&ws.conn, &db.id, databases::CreateRecord { template_id: Some(tpl.id.clone()), ..Default::default() }).unwrap();
    assert_eq!(rec.values[&status.id], json!(done));
    let all = databases::query(&ws.conn, &db.id, &Default::default()).unwrap();
    assert_eq!(all.total, 1, "templates are not listed as records");
}

#[test]
fn ac14_mcp_scope_is_enforced() {
    let (_d, ws) = ws();
    let shared = page(&ws, "Shared", None);
    let child = page(&ws, "Child", Some(&shared.id));
    let private = page(&ws, "Private", None);
    assert!(!Policy::load(&ws.conn).unwrap().can_read_page(&ws.conn, &shared.id).unwrap(), "off by default");
    settings::set_user(&ws.conn, "mcp", &json!({ "enabled": true, "scope": "selected", "allowedIds": [shared.id] })).unwrap();
    let policy = Policy::load(&ws.conn).unwrap();
    assert!(policy.can_read_page(&ws.conn, &child.id).unwrap());
    assert!(!policy.can_read_page(&ws.conn, &private.id).unwrap());

    let db = pages::create(&ws.conn, CreatePage { kind: "database".into(), title: "Secret DB".into(), parent_id: Some(shared.id.clone()), ..Default::default() }).unwrap();
    assert!(policy.can_read_page(&ws.conn, &db.id).unwrap());
    databases::set_mcp_access(&ws.conn, &db.id, "deny").unwrap();
    assert!(!policy.can_read_page(&ws.conn, &db.id).unwrap(), "per-database opt-out wins");
}

#[test]
fn property_type_change_reports_loss() {
    let (_d, ws) = ws();
    let db = pages::create(&ws.conn, CreatePage { kind: "database".into(), title: "T".into(), ..Default::default() }).unwrap();
    let text = databases::add_property(&ws.conn, &db.id, "Note", "text").unwrap();
    for v in ["12", "abc"] {
        let mut values = serde_json::Map::new();
        values.insert(text.id.clone(), json!(v));
        databases::create_record(&ws.conn, &db.id, databases::CreateRecord { title: v.into(), values, ..Default::default() }).unwrap();
    }
    let preview = databases::change_property_type(&ws.conn, &text.id, "number", false).unwrap();
    assert_eq!(preview["lost"], json!(1));
    assert_eq!(databases::get(&ws.conn, &db.id).unwrap().properties.iter().find(|p| p.id == text.id).unwrap().ptype, "text", "preview changes nothing");
    databases::change_property_type(&ws.conn, &text.id, "number", true).unwrap();
    let res = databases::query(&ws.conn, &db.id, &Default::default()).unwrap();
    assert_eq!(res.records.iter().filter(|r| r.values.contains_key(&text.id)).count(), 1);
}

#[test]
fn trash_and_restore_subtree() {
    let (_d, ws) = ws();
    let a = page(&ws, "A", None);
    let b = page(&ws, "B", Some(&a.id));
    pages::trash(&ws.conn, &a.id).unwrap();
    let trash = pages::list_trash(&ws.conn).unwrap();
    assert_eq!(trash.len(), 1, "children are not listed separately");
    pages::restore(&ws.conn, &a.id).unwrap();
    assert!(pages::get_meta(&ws.conn, &b.id).unwrap().deleted_at.is_none());
    assert!(pages::delete_forever(&ws.conn, &a.id).is_err(), "must be in trash first");
}

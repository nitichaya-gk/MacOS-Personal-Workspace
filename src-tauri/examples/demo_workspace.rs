//! Creates a demo workspace for manual testing and MCP smoke tests:
//! `cargo run --example demo_workspace -- /path/to/folder`
use personal_workspace_lib::application::pages::{self, CreatePage};
use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::application::{databases, settings, tasks};
use serde_json::json;

fn main() {
    let root = std::env::args().nth(1).expect("usage: demo_workspace <folder>");
    let ws = Workspace::open(std::path::Path::new(&root)).unwrap();
    let c = &ws.conn;
    let project = pages::create(c, CreatePage { kind: "project".into(), title: "แอปจัดการงาน".into(), icon: Some("🚀".into()), ..Default::default() }).unwrap();
    let spec = pages::create(c, CreatePage { kind: "page".into(), title: "สเปกระบบ".into(), parent_id: Some(project.id.clone()), ..Default::default() }).unwrap();
    pages::save_blocks(c, &spec.id, &json!([
        { "id": "h1", "type": "heading", "props": { "level": 2 }, "content": [{ "type": "text", "text": "ภาพรวม", "styles": {} }], "children": [] },
        { "id": "p1", "type": "paragraph", "content": [{ "type": "text", "text": "ระบบนี้ช่วยวางแผนงานออกแบบประจำสัปดาห์", "styles": {} }], "children": [] }
    ])).unwrap();
    let db = pages::create(c, CreatePage { kind: "database".into(), title: "Backlog".into(), parent_id: Some(project.id.clone()), ..Default::default() }).unwrap();
    for t in ["ออกแบบหน้า Login", "เชื่อมต่อ API", "ทดสอบภาษาไทย"] {
        databases::create_record(c, &db.id, databases::CreateRecord { title: t.into(), ..Default::default() }).unwrap();
    }
    let canvas = pages::create(c, CreatePage { kind: "canvas".into(), canvas_kind: Some("diagram".into()), title: "สถาปัตยกรรม".into(), parent_id: Some(project.id.clone()), ..Default::default() }).unwrap();
    personal_workspace_lib::application::canvases::save(c, &canvas.id, &[
        json!({ "id": "ui", "type": "rectangle", "x": 0, "y": 0, "width": 120, "height": 60 }),
        json!({ "id": "ui-t", "type": "text", "containerId": "ui", "text": "Web UI", "x": 10, "y": 20 }),
        json!({ "id": "db", "type": "rectangle", "x": 300, "y": 0, "width": 120, "height": 60 }),
        json!({ "id": "arrow", "type": "arrow", "x": 120, "y": 30, "startBinding": { "elementId": "ui" }, "endBinding": { "elementId": "db" } }),
    ], &json!({})).unwrap();
    tasks::create(c, &json!({ "title": "ส่งร่างแรก", "dueDate": tasks::today(c), "projectId": project.id })).unwrap();
    let private = pages::create(c, CreatePage { kind: "page".into(), title: "บันทึกส่วนตัว".into(), ..Default::default() }).unwrap();
    settings::set_user(c, "mcp", &json!({ "enabled": true, "scope": "selected", "allowedIds": [project.id] })).unwrap();
    println!("{}", json!({ "project": project.id, "spec": spec.id, "database": db.id, "canvas": canvas.id, "private": private.id }));
}

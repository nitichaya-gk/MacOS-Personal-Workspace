//! PRD phase 4: static site export (AC-19) and background-agent safety (no double announcements).

use personal_workspace_lib::application::pages::{self, CreatePage};
use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::application::{assets, settings, site, tasks};
use personal_workspace_lib::integrations::digest;
use serde_json::json;
use std::collections::HashMap;

fn ws() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(&dir.path().join("ws")).unwrap();
    (dir, ws)
}

#[test]
fn ac19_static_site_contains_only_selected_pages() {
    let (dir, ws) = ws();
    let c = &ws.conn;
    let public = pages::create(c, CreatePage { kind: "page".into(), title: "หน้าสาธารณะ".into(), ..Default::default() }).unwrap();
    let child = pages::create(c, CreatePage { kind: "page".into(), title: "หน้าย่อย".into(), parent_id: Some(public.id.clone()), ..Default::default() }).unwrap();
    let secret = pages::create(c, CreatePage { kind: "page".into(), title: "ความลับ".into(), ..Default::default() }).unwrap();
    let img = assets::upload(&ws, "pic.png", "image/png", Some(&public.id), b"png").unwrap();
    let unused = assets::upload(&ws, "secret.png", "image/png", Some(&secret.id), b"png").unwrap();
    pages::save_blocks(c, &public.id, &json!([
        { "id": "1", "type": "paragraph", "content": [{ "type": "text", "text": "ดู ", "styles": {} }, { "type": "pageMention", "props": { "pageId": child.id } }, { "type": "text", "text": " และ ", "styles": {} }, { "type": "pageMention", "props": { "pageId": secret.id } }], "children": [] },
        { "id": "2", "type": "image", "props": { "url": img.url, "caption": "<script>x</script>" }, "children": [] }
    ])).unwrap();
    pages::save_blocks(c, &secret.id, &json!([{ "id": "s", "type": "paragraph", "content": [{ "type": "text", "text": "รหัสผ่านลับ", "styles": {} }], "children": [] }])).unwrap();

    let out = site::export(&ws, &[public.id.clone()], true, "My Site", dir.path(), &HashMap::new()).unwrap();
    let root = std::path::PathBuf::from(out["path"].as_str().unwrap());
    assert_eq!(out["pages"], 2);
    let html = std::fs::read_to_string(root.join(format!("{}.html", public.id))).unwrap();
    assert!(html.contains(&format!("href=\"{}.html\"", child.id)), "link to exported child");
    assert!(html.contains("@ความลับ") && !html.contains(&format!("{}.html", secret.id)), "secret is text only");
    assert!(!html.contains("<script>"), "content is escaped");
    assert!(root.join("assets").join(&img.id).exists());
    assert!(!root.join("assets").join(&unused.id).exists());
    assert!(!root.join(format!("{}.html", secret.id)).exists());
    let all: String = std::fs::read_dir(&root).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "html")).map(|e| std::fs::read_to_string(e.path()).unwrap()).collect();
    assert!(!all.contains("รหัสผ่านลับ"));
    assert!(root.join("index.html").exists() && root.join("style.css").exists());
    assert!(site::export(&ws, &[public.id.clone()], true, "My Site", dir.path(), &HashMap::new()).is_err(), "never overwrites");
}

#[test]
fn daily_summary_is_claimed_once_across_app_and_agent() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    settings::set_user(c, "timezone", &json!("UTC")).unwrap();
    settings::set_user(c, "dailySummary", &json!({ "enabled": true, "time": "00:00" })).unwrap();
    tasks::create(c, &json!({ "title": "x", "dueDate": tasks::today(c) })).unwrap();
    let first = digest::collect(c);
    assert_eq!(first.iter().filter(|n| n.title == "Today").count(), 1);
    // A second process (the agent) ticking right after must not repeat it.
    let ws2 = Workspace::open(&ws.root).unwrap();
    assert_eq!(digest::collect(&ws2.conn).iter().filter(|n| n.title == "Today").count(), 0);
}

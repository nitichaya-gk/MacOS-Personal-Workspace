//! Default structure and starter content for a brand-new workspace.

use super::pages::{self, CreatePage};
use crate::domain::{new_id, now};
use crate::error::Result;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

fn txt(s: &str) -> Value {
    json!([{ "type": "text", "text": s, "styles": {} }])
}
fn p(s: &str) -> Value {
    json!({ "id": new_id(), "type": "paragraph", "content": txt(s), "children": [] })
}
fn h(level: u8, s: &str) -> Value {
    json!({ "id": new_id(), "type": "heading", "props": { "level": level }, "content": txt(s), "children": [] })
}
fn li(s: &str) -> Value {
    json!({ "id": new_id(), "type": "bulletListItem", "content": txt(s), "children": [] })
}
fn todo(s: &str) -> Value {
    json!({ "id": new_id(), "type": "checkListItem", "props": { "checked": false }, "content": txt(s), "children": [] })
}
fn callout(emoji: &str, s: &str) -> Value {
    json!({ "id": new_id(), "type": "callout", "props": { "emoji": emoji }, "content": txt(s), "children": [] })
}
fn divider() -> Value {
    json!({ "id": new_id(), "type": "divider", "children": [] })
}

pub fn seed(conn: &Connection) -> Result<(String, String)> {
    let id = new_id();
    let name = "My Workspace".to_string();
    let ts = now();
    conn.execute("INSERT INTO workspaces(id, name, created_at) VALUES (?1,?2,?3)", params![id, name, ts])?;
    let sections = [("Projects", "📁", "projects"), ("Knowledge Base", "📚", "knowledge"), ("Databases", "🗂️", "databases"), ("Templates", "🧩", "templates")];
    let mut ids = vec![];
    for (i, (n, icon, kind)) in sections.iter().enumerate() {
        let sid = new_id();
        conn.execute(
            "INSERT INTO sections(id, name, icon, kind, position, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
            params![sid, n, icon, kind, i as f64, ts],
        )?;
        ids.push(sid);
    }
    let (knowledge, templates) = (ids[1].clone(), ids[3].clone());

    let welcome = pages::create(conn, CreatePage { section_id: Some(knowledge), kind: "page".into(), title: "Getting started".into(), icon: Some("👋".into()), ..Default::default() })?;
    pages::save_blocks(
        conn,
        &welcome.id,
        &json!([
            callout("💡", "Everything here is stored on this Mac only. It works offline, and nothing is sent anywhere unless you export it or connect an MCP client."),
            h(2, "Write"),
            p("Type / for the block menu (headings, lists, to-dos, toggles, tables, code, Mermaid, callouts, images and files, embeds). Type @ to link another page. Drag the ⋮⋮ handle to reorder blocks."),
            h(2, "Organise"),
            li("Projects hold an overview plus their documents, tasks, diagrams, wireframes and assets."),
            li("Databases have Table, Board, Calendar and List views with filters and sorting."),
            li("Templates create fresh copies; the template itself never changes."),
            h(2, "Plan"),
            p("Tasks with a due date show up in Today. Give a task a reminder time and macOS will notify you."),
            todo("Create your first project"),
            todo("Add a task due today"),
            todo("Make a backup in Settings → Backup"),
            divider(),
            p("Shortcuts: ⌘K command palette · ⌘P search · ⌘N new page · ⌘\\ toggle sidebar · ⌘[ / ⌘] back and forward."),
        ]),
    )?;

    let tpl = |title: &str, icon: &str, kind: &str, blocks: Value| -> Result<()> {
        let page = pages::create(conn, CreatePage { section_id: Some(templates.clone()), kind: kind.into(), title: title.into(), icon: Some(icon.into()), ..Default::default() })?;
        pages::save_blocks(conn, &page.id, &blocks)?;
        Ok(())
    };
    tpl("Project", "🚀", "project", json!([
        h(2, "Goal"), p("What does done look like?"),
        h(2, "Scope"), li("In scope"), li("Out of scope"),
        h(2, "Milestones"), todo("Kick-off"), todo("First draft"), todo("Review"),
        h(2, "Notes"), p(""),
    ]))?;
    tpl("Meeting Notes", "🗓️", "page", json!([
        p("Date: · Attendees:"),
        h(2, "Agenda"), li(""),
        h(2, "Notes"), p(""),
        h(2, "Decisions"), li(""),
        h(2, "Action items"), todo(""),
    ]))?;
    tpl("Daily Notes", "📝", "page", json!([
        h(2, "Top 3 today"), todo(""), todo(""), todo(""),
        h(2, "Log"), p(""),
        h(2, "Tomorrow"), li(""),
    ]))?;
    tpl("Task", "✅", "page", json!([
        callout("🎯", "Outcome: what needs to be true when this is finished?"),
        h(2, "Steps"), todo(""),
        h(2, "Context & links"), p(""),
    ]))?;
    tpl("Design Brief", "🎨", "page", json!([
        h(2, "Problem"), p(""),
        h(2, "Users"), p(""),
        h(2, "Requirements"), li(""),
        h(2, "Constraints"), li(""),
        h(2, "Success metrics"), li(""),
        h(2, "Wireframes"), p("Create a wireframe canvas under this page and embed it here with /canvas."),
    ]))?;
    Ok((id, name))
}

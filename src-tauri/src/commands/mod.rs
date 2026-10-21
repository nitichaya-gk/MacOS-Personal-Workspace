//! Tauri commands. The UI talks to the application layer through one `rpc` entry point
//! (method name + JSON params), plus a few commands that need the window or raw bytes.

use crate::application::workspace::{self, Workspace};
use crate::application::{assets, canvases, databases, export, finance, goals, habits, mcp_access, pages, reviews, sections, settings, skills, tasks, templates};
use crate::error::{invalid, AppError, Result};
use crate::infrastructure::{backup, filesystem, search};
use crate::AppState;
use base64::Engine;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::State;

fn arg<T: DeserializeOwned>(p: &Value, key: &str) -> Result<T> {
    serde_json::from_value(p.get(key).cloned().unwrap_or(Value::Null)).map_err(|e| AppError::Invalid(format!("{key}: {e}")))
}

fn opt<T: DeserializeOwned>(p: &Value, key: &str) -> Option<T> {
    p.get(key).filter(|v| !v.is_null()).and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn s<'a>(p: &'a Value, key: &str) -> Result<&'a str> {
    p.get(key).and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid(format!("missing {key}")))
}

fn to<T: serde::Serialize>(v: T) -> Result<Value> {
    Ok(serde_json::to_value(v)?)
}

fn ok() -> Result<Value> {
    Ok(json!({ "ok": true }))
}

fn user_path(p: &Value, key: &str) -> Result<PathBuf> {
    let raw = s(p, key)?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return invalid("path must be absolute");
    }
    Ok(path)
}

#[tauri::command]
pub async fn rpc(state: State<'_, AppState>, method: String, params: Value) -> std::result::Result<Value, AppError> {
    let mut guard = state.ws.lock().map_err(|_| AppError::Invalid("workspace lock poisoned".into()))?;
    let result = dispatch(&mut guard, &method, &params);
    if method.starts_with("workspace.switch") || method == "workspace.create" {
        *state.root.write().unwrap() = guard.root.clone();
    }
    if let Err(e) = &result {
        eprintln!("rpc {method} failed: {e}");
    }
    result
}

pub fn dispatch(ws: &mut Workspace, method: &str, p: &Value) -> Result<Value> {
    let c = &ws.conn;
    match method {
        // workspace
        "workspace.info" => ws.info(),
        "workspace.rename" => {
            ws.rename(s(p, "name")?)?;
            ws.info()
        }
        "workspace.switch" | "workspace.create" => {
            let root = user_path(p, "root")?;
            if method == "workspace.switch" && !root.join("workspace.sqlite").exists() {
                return invalid("that folder does not contain a workspace");
            }
            let next = Workspace::open(&root)?;
            workspace::remember_workspace(&root)?;
            *ws = next;
            ws.info()
        }
        "workspace.rebuildIndex" => Ok(json!({ "indexed": pages::rebuild_index(c)? })),
        "workspace.integrity" => backup::integrity(c, &ws.root),

        // sections
        "sections.list" => to(sections::list(c)?),
        "sections.create" => to(sections::create(c, s(p, "name")?, opt::<String>(p, "icon").as_deref())?),
        "sections.update" => {
            sections::update(c, s(p, "id")?, opt::<String>(p, "name").as_deref(), opt::<String>(p, "icon").as_deref(), opt(p, "position"))?;
            to(sections::list(c)?)
        }
        "sections.delete" => {
            sections::delete(c, s(p, "id")?)?;
            ok()
        }

        // pages
        "pages.tree" => to(pages::list_tree(c)?),
        "pages.recent" => to(pages::list_recent(c, opt(p, "limit").unwrap_or(12))?),
        "pages.trash" => Ok(json!({ "pages": pages::list_trash(c)?, "tasks": tasks::list_trash(c)? })),
        "pages.archived" => to(pages::list_archive(c)?),
        "pages.open" => to(pages::open(c, s(p, "id")?)?),
        "pages.meta" => to(pages::get_meta(c, s(p, "id")?)?),
        "pages.subtree" => {
            let ids = pages::subtree_ids(c, s(p, "id")?)?;
            let mut out = vec![];
            for id in ids.iter().skip(1) {
                let m = pages::get_meta(c, id)?;
                if m.deleted_at.is_none() && m.kind != "record" {
                    out.push(m);
                }
            }
            to(out)
        }
        "pages.create" => to(pages::create(c, serde_json::from_value(p.clone())?)?),
        "pages.update" => to(pages::update(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({})))?),
        "pages.move" => to(pages::move_page(c, s(p, "id")?, opt::<String>(p, "parentId").as_deref(), opt::<String>(p, "sectionId").as_deref(), opt(p, "index"))?),
        "pages.duplicate" => to(pages::duplicate(c, s(p, "id")?)?),
        "pages.archive" => {
            pages::archive(c, s(p, "id")?)?;
            ok()
        }
        "pages.unarchive" => {
            pages::unarchive(c, s(p, "id")?)?;
            ok()
        }
        "pages.trashPage" => {
            pages::trash(c, s(p, "id")?)?;
            ok()
        }
        "pages.restore" => to(pages::restore(c, s(p, "id")?)?),
        "pages.deleteForever" => {
            pages::delete_forever(c, s(p, "id")?)?;
            ok()
        }
        "pages.emptyTrash" => Ok(json!({ "deleted": pages::empty_trash(c)? })),
        "pages.saveBlocks" => Ok(json!({ "updatedAt": pages::save_blocks(c, s(p, "id")?, p.get("blocks").unwrap_or(&json!([])))? })),
        "pages.backlinks" => to(pages::backlinks(c, s(p, "id")?)?),
        "pages.ofKind" => to(pages::query_metas(
            c,
            "WHERE p.kind=?1 AND p.deleted_at IS NULL AND p.is_template=0 ORDER BY p.title",
            [s(p, "kind")?],
        )?),

        // databases
        "databases.get" => to(databases::get(c, s(p, "id")?)?),
        "databases.query" => to(databases::query(c, s(p, "id")?, &opt(p, "spec").unwrap_or_default())?),
        "databases.addProperty" => to(databases::add_property(c, s(p, "id")?, s(p, "name")?, s(p, "type")?)?),
        "databases.updateProperty" => to(databases::update_property(c, s(p, "propertyId")?, p.get("patch").unwrap_or(&json!({})))?),
        "databases.addOption" => databases::add_option(c, s(p, "propertyId")?, s(p, "name")?),
        "databases.deleteProperty" => {
            databases::delete_property(c, s(p, "propertyId")?)?;
            ok()
        }
        "databases.changePropertyType" => databases::change_property_type(c, s(p, "propertyId")?, s(p, "type")?, opt(p, "apply").unwrap_or(false)),
        "databases.addView" => to(databases::add_view(c, s(p, "id")?, opt::<String>(p, "name").as_deref().unwrap_or(""), s(p, "type")?)?),
        "databases.updateView" => to(databases::update_view(c, s(p, "viewId")?, p.get("patch").unwrap_or(&json!({})))?),
        "databases.deleteView" => {
            databases::delete_view(c, s(p, "viewId")?)?;
            ok()
        }
        "databases.createRecord" => to(databases::create_record(c, s(p, "id")?, opt(p, "record").unwrap_or_default())?),
        "databases.getRecord" => to(databases::get_record(c, s(p, "recordId")?)?),
        "databases.recordContext" => {
            let rid = s(p, "recordId")?;
            match databases::database_of_record(c, rid)? {
                Some(db) => Ok(json!({ "database": databases::get(c, &db)?, "record": databases::get_record(c, rid)? })),
                None => Ok(Value::Null),
            }
        }
        "databases.setValue" => to(databases::set_value(c, s(p, "recordId")?, s(p, "propertyId")?, p.get("value").unwrap_or(&Value::Null))?),
        "databases.moveRecord" => {
            databases::move_record(c, s(p, "recordId")?, opt::<String>(p, "beforeId").as_deref())?;
            ok()
        }
        "databases.setMcpAccess" => {
            databases::set_mcp_access(c, s(p, "id")?, s(p, "access")?)?;
            ok()
        }
        "databases.importCsv" => to(databases::import_csv(c, s(p, "csv")?, s(p, "title")?, opt(p, "sectionId"), opt(p, "parentId"))?),

        // tasks
        "tasks.list" => to(tasks::list(
            c,
            opt::<String>(p, "view").as_deref().unwrap_or("open"),
            opt::<String>(p, "projectId").as_deref(),
            opt::<String>(p, "pageId").as_deref(),
            opt(p, "limit").unwrap_or(500),
        )?),
        "tasks.counts" => tasks::counts(c),
        "tasks.get" => to(tasks::get(c, s(p, "id")?)?),
        "tasks.create" => to(tasks::create(c, p.get("task").unwrap_or(&json!({})))?),
        "tasks.update" => to(tasks::update(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({})))?),
        "tasks.delete" => {
            tasks::delete(c, s(p, "id")?)?;
            ok()
        }
        "tasks.restore" => to(tasks::restore(c, s(p, "id")?)?),
        "tasks.deleteForever" => {
            tasks::delete_forever(c, s(p, "id")?)?;
            ok()
        }
        "tasks.missed" => to(tasks::list_missed(c)?),
        "tasks.dismissMissed" => {
            tasks::dismiss_missed(c)?;
            ok()
        }

        // canvases
        "canvases.load" => canvases::load(ws, s(p, "id")?),
        "canvases.save" => {
            let elements: Vec<Value> = arg(p, "elements")?;
            Ok(json!({ "updatedAt": canvases::save(c, s(p, "id")?, &elements, p.get("appState").unwrap_or(&json!({})))? }))
        }
        "canvases.saveFile" => {
            canvases::save_file(ws, s(p, "id")?, s(p, "fileId")?, s(p, "mimeType")?, s(p, "dataURL")?)?;
            ok()
        }

        // assets
        "assets.get" => to(assets::get(ws, s(p, "id")?)?),
        "assets.listFor" => to(assets::list_for(ws, s(p, "pageId")?)?),

        // templates
        "templates.list" => to(templates::list(c)?),
        "templates.instantiate" => to(templates::instantiate(
            c,
            s(p, "templateId")?,
            opt::<String>(p, "parentId").as_deref(),
            opt::<String>(p, "sectionId").as_deref(),
            opt::<String>(p, "title").as_deref(),
        )?),
        "templates.saveAs" => to(templates::save_as_template(c, s(p, "pageId")?)?),

        // search
        "search.query" => to(search::search(c, s(p, "query")?, &opt(p, "filters").unwrap_or_default())?),

        // settings
        "settings.get" => settings::all(c),
        "settings.set" => settings::set_user(c, s(p, "key")?, p.get("value").unwrap_or(&Value::Null)),

        // backup / restore
        "backup.list" => to(backup::list(&ws.root)?),
        "backup.create" => {
            let info = backup::create(c, &ws.root, &ws.manifest()?, "")?;
            let keep = settings::get(c, "backup")?.get("keep").and_then(|k| k.as_u64()).unwrap_or(10) as usize;
            backup::prune(&ws.root, keep)?;
            Ok(info)
        }
        "backup.exportZip" => backup::export_zip(c, &ws.root, &ws.manifest()?, &user_path(p, "dest")?),
        "backup.restore" => {
            let dest = user_path(p, "dest")?;
            if dest.starts_with(&ws.root) {
                return invalid("restore into a folder outside the current workspace");
            }
            backup::restore(&user_path(p, "source")?, &dest)
        }

        // export
        "export.pageMarkdown" => export::export_page_markdown(ws, s(p, "id")?, &user_path(p, "dest")?),
        "export.markdownText" => {
            let md = export::page_markdown(c, s(p, "id")?, None)?;
            Ok(json!({ "markdown": md.text, "warnings": md.warnings }))
        }
        "export.database" => export::export_database(ws, s(p, "id")?, s(p, "format")?, &user_path(p, "dest")?),
        "export.writeFile" => {
            let bytes = match (opt::<String>(p, "text"), opt::<String>(p, "base64")) {
                (Some(t), _) => t.into_bytes(),
                (None, Some(b)) => base64::engine::general_purpose::STANDARD.decode(b).map_err(|_| AppError::Invalid("bad base64".into()))?,
                _ => return invalid("nothing to write"),
            };
            let dest = user_path(p, "dest")?;
            filesystem::atomic_write(&dest, &bytes)?;
            Ok(json!({ "path": dest.to_string_lossy() }))
        }
        "import.readText" => {
            let path = user_path(p, "path")?;
            let meta = std::fs::metadata(&path)?;
            if meta.len() > 50 * 1024 * 1024 {
                return invalid("file is too large to import (50 MB max)");
            }
            Ok(json!({ "text": String::from_utf8_lossy(&std::fs::read(&path)?), "name": path.file_stem().map(|s| s.to_string_lossy().to_string()) }))
        }

        // goals & roadmap
        "goals.list" => to(goals::list(c, opt::<String>(p, "domain").as_deref(), opt(p, "includeClosed").unwrap_or(false))?),
        "goals.options" => to(goals::options(c)?),
        "goals.detail" => goals::detail(c, s(p, "id")?),
        "goals.create" => goals::create(c, p.get("goal").unwrap_or(&json!({}))),
        "goals.update" => goals::update(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({}))),
        "goals.delete" => {
            goals::delete(c, s(p, "id")?)?;
            ok()
        }
        "goals.restore" => goals::restore(c, s(p, "id")?),
        "goals.deleteForever" => {
            goals::delete_forever(c, s(p, "id")?)?;
            ok()
        }
        "goals.trash" => to(c.prepare("SELECT id, title, deleted_at FROM goals WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC")?
            .query_map([], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "deletedAt": r.get::<_, String>(2)? })))?
            .collect::<std::result::Result<Vec<_>, _>>()?),
        "goals.createExample" => goals::create_example(c),
        "goals.addRequirement" => goals::add_requirement(c, s(p, "goalId")?, p.get("requirement").unwrap_or(&json!({}))),
        "goals.updateRequirement" => goals::update_requirement(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({}))),
        "goals.deleteRequirement" => {
            goals::delete_requirement(c, s(p, "id")?)?;
            ok()
        }
        "goals.addMilestone" => goals::add_milestone(c, s(p, "goalId")?, p.get("milestone").unwrap_or(&json!({}))),
        "goals.updateMilestone" => goals::update_milestone(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({}))),
        "goals.deleteMilestone" => {
            goals::delete_milestone(c, s(p, "id")?)?;
            ok()
        }
        "goals.addMilestoneTask" => to(goals::add_milestone_task(c, s(p, "milestoneId")?, p.get("task").unwrap_or(&json!({})))?),
        "goals.setSkill" => {
            goals::set_goal_skill(c, s(p, "goalId")?, s(p, "skillId")?, opt(p, "targetLevel"))?;
            ok()
        }
        "goals.drafts" => to(goals::list_drafts(c, opt::<String>(p, "goalId").as_deref())?),
        "goals.resolveDraft" => goals::resolve_draft(c, s(p, "id")?, opt(p, "apply").unwrap_or(false)),

        // skills, evidence, focus
        "skills.list" => to(skills::list(c)?),
        "skills.get" => skills::get(c, s(p, "id")?),
        "skills.create" => skills::create(c, p.get("skill").unwrap_or(&json!({}))),
        "skills.update" => skills::update(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({}))),
        "skills.delete" => {
            skills::delete(c, s(p, "id")?)?;
            ok()
        }
        "skills.evidence" => to(skills::evidence_for_skill(c, s(p, "skillId")?)?),
        "skills.addEvidence" => skills::add_evidence(c, p.get("evidence").unwrap_or(&json!({}))),
        "skills.deleteEvidence" => {
            skills::delete_evidence(c, s(p, "id")?)?;
            ok()
        }
        "focus.running" => to(skills::running_session(c)?),
        "focus.start" => skills::start_session(c, p.get("session").unwrap_or(&json!({}))),
        "focus.stop" => skills::stop_session(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({}))),
        "focus.discard" => {
            skills::discard_session(c, s(p, "id")?)?;
            ok()
        }

        // habits
        "habits.list" => to(habits::list(c, opt(p, "includeArchived").unwrap_or(false))?),
        "habits.create" => habits::create(c, p.get("habit").unwrap_or(&json!({}))),
        "habits.update" => habits::update(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({}))),
        "habits.delete" => {
            habits::delete(c, s(p, "id")?)?;
            ok()
        }
        "habits.log" => habits::log(c, s(p, "habitId")?, s(p, "date")?, opt::<String>(p, "status").as_deref(), opt(p, "value"), opt::<String>(p, "note").as_deref().unwrap_or("")),

        // finance
        "finance.accounts" => to(finance::accounts(c, opt(p, "includeArchived").unwrap_or(false))?),
        "finance.createAccount" => finance::create_account(c, p.get("account").unwrap_or(&json!({}))),
        "finance.updateAccount" => {
            finance::update_account(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({})))?;
            ok()
        }
        "finance.categories" => to(finance::categories(c)?),
        "finance.createCategory" => finance::create_category(c, s(p, "name")?, s(p, "kind")?),
        "finance.addTransaction" => finance::add_transaction(c, p.get("transaction").unwrap_or(&json!({}))),
        "finance.updateTransaction" => finance::update_transaction(c, s(p, "id")?, p.get("patch").unwrap_or(&json!({}))),
        "finance.deleteTransaction" => {
            finance::delete_transaction(c, s(p, "id")?)?;
            ok()
        }
        "finance.restoreTransaction" => {
            finance::restore_transaction(c, s(p, "id")?)?;
            ok()
        }
        "finance.transactions" => to(finance::list_transactions(c, opt::<String>(p, "month").as_deref(), opt::<String>(p, "accountId").as_deref(), opt::<String>(p, "goalId").as_deref(), opt(p, "limit").unwrap_or(500))?),
        "finance.month" => finance::month_summary(c, s(p, "month")?),
        "finance.setBudget" => {
            finance::set_budget(c, s(p, "categoryId")?, p)?;
            ok()
        }
        "finance.savingsGoals" => to(finance::savings_goals(c, None)?),
        "finance.createSavingsGoal" => finance::create_savings_goal(c, p.get("savingsGoal").unwrap_or(&json!({}))),
        "finance.deleteSavingsGoal" => {
            finance::delete_savings_goal(c, s(p, "id")?)?;
            ok()
        }
        "finance.allocate" => finance::allocate(c, s(p, "id")?, p),
        "finance.audit" => to(finance::audit_trail(c, opt(p, "limit").unwrap_or(100))?),

        // today & reviews
        "growth.today" => reviews::today_plan(c),
        "reviews.currentWeek" => Ok(json!({ "period": reviews::current_week(c) })),
        "reviews.get" => reviews::get(c, &opt::<String>(p, "period").unwrap_or_else(|| reviews::current_week(c))),
        "reviews.save" => reviews::save(c, s(p, "period")?, p.get("review").unwrap_or(&json!({}))),
        "reviews.list" => to(reviews::list(c)?),

        // MCP
        "mcp.status" => {
            let exe = std::env::current_exe()?.to_string_lossy().to_string();
            Ok(json!({
                "executable": exe,
                "args": ["--mcp"],
                "claudeDesktop": { "mcpServers": { "personal-workspace": { "command": exe, "args": ["--mcp"] } } },
                "claudeCode": format!("claude mcp add personal-workspace -- \"{exe}\" --mcp"),
                "codex": format!("[mcp_servers.personal-workspace]\ncommand = \"{exe}\"\nargs = [\"--mcp\"]"),
                "audit": mcp_access::audit_log(c, 50)?,
            }))
        }

        _ => invalid(format!("unknown method {method}")),
    }
}

/// Raw-bytes upload (avoids JSON-encoding files). Metadata travels in headers.
#[tauri::command]
pub fn upload_asset(state: State<'_, AppState>, request: tauri::ipc::Request<'_>) -> std::result::Result<Value, AppError> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return invalid("expected file bytes");
    };
    let header = |k: &str| request.headers().get(k).and_then(|v| v.to_str().ok()).map(percent_decode).unwrap_or_default();
    let ws = state.ws.lock().map_err(|_| AppError::Invalid("workspace lock poisoned".into()))?;
    let name = header("x-file-name");
    let page = header("x-page-id");
    to(assets::upload(&ws, if name.is_empty() { "file" } else { &name }, &header("x-mime"), (!page.is_empty()).then_some(page.as_str()), bytes)?)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[tauri::command]
pub fn print_page(window: tauri::WebviewWindow) -> std::result::Result<(), AppError> {
    window.print().map_err(|e| AppError::Invalid(e.to_string()))
}

/// Opens http(s)/mailto links in the default browser, or reveals a file in Finder.
#[tauri::command]
pub fn open_external(target: String, reveal: Option<bool>) -> std::result::Result<(), AppError> {
    let mut cmd = std::process::Command::new("/usr/bin/open");
    if reveal == Some(true) {
        if !PathBuf::from(&target).is_absolute() {
            return invalid("path must be absolute");
        }
        cmd.arg("-R");
    } else if !(target.starts_with("https://") || target.starts_with("http://") || target.starts_with("mailto:")) {
        return invalid("only web and mail links can be opened");
    }
    cmd.arg(&target).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes_thai_header() {
        assert_eq!(super::percent_decode("%E0%B8%A3%E0%B8%B9%E0%B8%9B.png"), "รูป.png");
    }
}

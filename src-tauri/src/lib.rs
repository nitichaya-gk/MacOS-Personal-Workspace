#![recursion_limit = "512"]

pub mod application;
pub mod commands;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod integrations;

use application::workspace::{self, Workspace};
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use tauri::http::{Response, StatusCode};
use tauri::Manager;

pub struct AppState {
    pub ws: Mutex<Workspace>,
    /// Current workspace folder, readable without the database lock (used by the asset protocol).
    pub root: RwLock<PathBuf>,
}

pub fn run() {
    let root = workspace::resolve_workspace_root();
    let ws = match Workspace::open(&root) {
        Ok(ws) => ws,
        Err(e) => {
            // Fall back to the default location if a configured workspace is missing or unreadable.
            eprintln!("could not open workspace at {}: {e}", root.display());
            Workspace::open(&workspace::default_workspace_root()).expect("could not open the default workspace")
        }
    };
    let _ = workspace::remember_workspace(&ws.root);
    let state = AppState { root: RwLock::new(ws.root.clone()), ws: Mutex::new(ws) };

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .register_uri_scheme_protocol("wsasset", |ctx, request| {
            let state = ctx.app_handle().state::<AppState>();
            let root = state.root.read().unwrap().clone();
            let id = request.uri().path().trim_start_matches('/').to_string();
            match application::assets::read(&root, &id) {
                Ok((bytes, mime)) => Response::builder()
                    .header("Content-Type", mime)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Cache-Control", "max-age=31536000, immutable")
                    .body(bytes)
                    .unwrap(),
                Err(_) => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap(),
            }
        })
        .setup(|app| {
            integrations::notifications::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::rpc,
            commands::upload_asset,
            commands::print_page,
            commands::open_external
        ])
        .run(tauri::generate_context!())
        .expect("error while running Personal Workspace");
}

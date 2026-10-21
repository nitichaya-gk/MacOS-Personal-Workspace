//! Headless background agent (`personal-workspace --agent`), started by a per-user LaunchAgent so
//! reminders, the daily summary, habit reminders and scheduled automations still arrive while the app
//! is closed. It uses the same services and database claims as the app, so nothing is announced twice.

use super::digest;
use crate::application::workspace::{app_support_dir, resolve_workspace_root, Workspace, APP_IDENTIFIER};
use crate::error::{AppError, Result};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

pub const AGENT_LABEL: &str = "com.personalworkspace.desktop.agent";

fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn notify(title: &str, body: &str) {
    let script = format!("display notification {} with title {}", applescript_string(body), applescript_string(title));
    let _ = Command::new("/usr/bin/osascript").args(["-e", &script]).status();
}

pub fn run() {
    let mut ws: Option<Workspace> = None;
    loop {
        let root = resolve_workspace_root();
        if root.join("workspace.sqlite").exists() {
            if ws.as_ref().map(|w| w.root != root).unwrap_or(true) {
                ws = Workspace::open(&root).map_err(|e| eprintln!("agent: cannot open workspace: {e}")).ok();
            }
            if let Some(w) = &ws {
                for n in digest::collect(&w.conn) {
                    notify(&n.title, &n.body);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(30));
    }
}

fn plist_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Library/LaunchAgents").join(format!("{AGENT_LABEL}.plist"))
}

fn uid() -> String {
    Command::new("/usr/bin/id").arg("-u").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

pub fn status() -> bool {
    plist_path().exists()
}

/// Installs and starts the LaunchAgent for the current app executable.
pub fn enable() -> Result<()> {
    let exe = std::env::current_exe()?;
    let logs = crate::application::workspace::logs_dir();
    std::fs::create_dir_all(&logs)?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>{AGENT_LABEL}</string>
  <key>ProgramArguments</key><array><string>{}</string><string>--agent</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>LowPriorityIO</key><true/>
  <key>StandardErrorPath</key><string>{}</string>
  <key>EnvironmentVariables</key><dict><key>PW_CONFIG_DIR</key><string>{}</string></dict>
</dict></plist>
"#,
        xml(&exe.to_string_lossy()),
        xml(&logs.join("agent.log").to_string_lossy()),
        xml(&app_support_dir().to_string_lossy()),
    );
    let path = plist_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    crate::infrastructure::filesystem::atomic_write(&path, plist.as_bytes())?;
    let _ = Command::new("/bin/launchctl").args(["bootout", &format!("gui/{}/{AGENT_LABEL}", uid())]).status();
    let ok = Command::new("/bin/launchctl").args(["bootstrap", &format!("gui/{}", uid())]).arg(&path).status()?;
    if !ok.success() {
        return Err(AppError::Invalid("macOS refused to start the background agent (launchctl bootstrap failed)".into()));
    }
    let _ = APP_IDENTIFIER;
    Ok(())
}

pub fn disable() -> Result<()> {
    let _ = Command::new("/bin/launchctl").args(["bootout", &format!("gui/{}/{AGENT_LABEL}", uid())]).status();
    let path = plist_path();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

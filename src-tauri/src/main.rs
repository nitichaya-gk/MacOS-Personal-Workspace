// Prevents an extra console window on Windows in release; harmless on macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `--mcp` turns the app binary into a stdio MCP server, so the bundled .app is all a client needs.
    if std::env::args().any(|a| a == "--mcp") {
        personal_workspace_lib::integrations::mcp::run_stdio();
        return;
    }
    if std::env::args().any(|a| a == "--agent") {
        personal_workspace_lib::integrations::agent::run();
        return;
    }
    personal_workspace_lib::run();
}

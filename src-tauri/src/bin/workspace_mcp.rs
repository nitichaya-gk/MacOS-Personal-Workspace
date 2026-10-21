//! Standalone MCP server binary for development: `cargo run --bin workspace_mcp [-- --workspace <path>]`.
fn main() {
    personal_workspace_lib::integrations::mcp::run_stdio();
}

# MCP server

- Transport: stdio, newline-delimited JSON-RPC 2.0. Start with `"<app>/Contents/MacOS/personal-workspace" --mcp` (or `npm run mcp` in development). `--workspace <folder>` or `PW_WORKSPACE` overrides which workspace is served; by default it follows the one open in the app.
- Off by default. Settings → MCP turns it on and chooses scope: **selected** sections/pages/projects/databases (descendants included) or **everything**. A database set to "Hide from MCP clients" is always excluded. Tasks are visible when their project is shared (or "Tasks without a project" is selected).
- Read-only in this phase. All tools are annotated `readOnlyHint: true`.
- Every call is written to `mcp_audit_logs` with client name, tool, target id, and ok / denied / error. Content is not logged.
- Tool output is the user's content and is labelled as data in the server instructions.

| Tool | Arguments | Returns |
| --- | --- | --- |
| `search_workspace` | `query`, `types?`, `projectId?`, `modifiedAfter?`, `limit≤50` | hits with id, type, title, snippet, project |
| `list_projects` | — | shared projects |
| `read_page` | `pageId`, `offset?` | title, kind, breadcrumb, children, Markdown (100k-char pages with `nextOffset`) |
| `query_database` | `databaseId`, `viewId?` or `filter?`/`sorts?`, `search?`, `offset?`, `limit≤100` | properties (with options), views, records with values by property name, `nextOffset` |
| `list_tasks` | `view?` today/overdue/upcoming/nodate/open/completed/all, `projectId?`, `limit≤200` | tasks with dates in the user's time zone |
| `read_canvas` | `canvasId` | objects (type, text, geometry, frame, link) and arrow connections |

Errors are returned as tool results with `isError: true` (e.g. `permission denied: … outside the scope shared with MCP clients`).

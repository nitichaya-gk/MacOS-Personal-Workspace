# MCP server

- Transport: stdio, newline-delimited JSON-RPC 2.0. Start with `"<app>/Contents/MacOS/personal-workspace" --mcp` (or `npm run mcp` in development). `--workspace <folder>` or `PW_WORKSPACE` overrides which workspace is served; by default it follows the one open in the app.
- Off by default. Settings → MCP turns it on and chooses scope: **selected** sections/pages/projects/databases (descendants included) or **everything**. A database set to "Hide from MCP clients" is always excluded. Tasks are visible when their project is shared (or "Tasks without a project" is selected).
- Read-only, except `suggest_goal_breakdown`, which only stores a draft. Nothing the AI proposes changes goals, tasks or finance until the user presses Apply in the goal's “AI drafts” tab.
- Growth domains are shared separately: `domain:goals`, `domain:skills`, `domain:habits` (or individual goal ids). Scope “everything” includes them. **Finance (`domain:finance`) is never included by “everything”** — it needs its own explicit grant with a confirmation.
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
| `list_goals` | `domain?`, `includeClosed?` | goals with status and rules-based health |
| `get_goal_roadmap` | `goalId` | requirements, milestones by phase with tasks, dependencies and allocations |
| `get_goal_health` | `goalId` | the four progress dimensions (separate, with formulas), health reasons, achievement blockers |
| `get_skill_gap` | `goalId?` | target vs evidence-assessed level, confidence, staleness |
| `get_today_plan` | — | today/overdue tasks, habits due, top goals, focus minutes (filtered by scope) |
| `get_habit_summary` | — | schedules, streaks, consistency |
| `get_finance_summary` | `month?` | monthly totals per currency (minor units), budgets, savings — sensitive, explicit grant only |
| `suggest_goal_breakdown` | `goalId`, `requirements[] → milestones[] → tasks[]`, `rationale?` | `{draftId, status: "pending"}` |

Errors are returned as tool results with `isError: true` (e.g. `permission denied: … outside the scope shared with MCP clients`).

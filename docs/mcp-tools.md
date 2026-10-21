# MCP server

- Transport: stdio, newline-delimited JSON-RPC 2.0. Start with `"<app>/Contents/MacOS/personal-workspace" --mcp` (or `npm run mcp` in development). `--workspace <folder>` or `PW_WORKSPACE` overrides which workspace is served; by default it follows the one open in the app.
- Off by default. Settings → MCP turns it on and chooses scope: **selected** sections/pages/projects/databases (descendants included) or **everything**. A database set to "Hide from MCP clients" is always excluded. Tasks are visible when their project is shared (or "Tasks without a project" is selected).
- **Reading and writing are separate grants.** Settings → MCP → “Allow changes” opens chosen pages/sections/projects, unassigned tasks (`tasks:unassigned`) or growth domains (`domain:goals|habits|skills`) for writes (`writeIds`). Everything else stays read-only. Page edits save a revision first (restorable from Version history), and `update_*` tools accept `dryRun` to preview.
- **Drafts for planning and money.** `suggest_goal_breakdown` and `create_transaction_draft` only store drafts; nothing changes until the user presses Apply (the goal's “AI drafts” tab, or the suggestions box in Finance, where the user also picks the account).
- Growth domains are shared separately: `domain:goals`, `domain:skills`, `domain:habits`, `domain:timetable` (or individual goal ids). Scope “everything” includes them. **Finance (`domain:finance`) is never included by “everything”** — it needs its own explicit grant with a confirmation, and it can never be opened for direct writes.
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
| `get_timetable` | `weekStart?` | the week's time blocks per day, planned vs actual focus minutes (needs `domain:timetable`) |
| `get_learning_plan` | — | courses, topics with status/prerequisites/next review, reviews due (needs `domain:skills`) |
| `suggest_weekly_review` | `period?` (e.g. `2026-W43`) | the locally generated weekly summary to draft reflections from; the user writes and saves the review |
| `get_habit_summary` | — | schedules, streaks, consistency |
| `get_finance_summary` | `month?` | monthly totals per currency (minor units), budgets, savings — sensitive, explicit grant only |
| `suggest_goal_breakdown` | `goalId`, `requirements[] → milestones[] → tasks[]`, `rationale?` | `{draftId, status: "pending"}` |
| `create_transaction_draft` | `type` income/expense, `amount`, `occurredOn?`, `merchantOrSource?`, `note?` | `{draftId, status: "pending"}`; finance grant required |

### Write tools (only inside what the user opened for changes)

| Tool | Arguments | Notes |
| --- | --- | --- |
| `create_page` | `title`, `parentId?` or `sectionId?`, `markdown?` | Markdown becomes editor blocks |
| `update_page` | `pageId`, `title?`, `appendMarkdown?` or `replaceMarkdown?`, `dryRun?` | revision saved before the change |
| `create_record` | `databaseId`, `title`, `values{propertyName: value}` | select/status by option name; date `YYYY-MM-DD` |
| `update_record` | `recordId`, `title?`, `values?`, `dryRun?` | formula/rollup properties are read-only |
| `create_task` / `update_task` | title, status, due date, priority, description, project | the project must be open for writes |
| `log_focus_session` | `minutes`, `taskId?`/`goalId?`, `note?` | needs `domain:goals` writes |
| `log_habit` | `habitId`, `status`, `date?` | needs `domain:habits` writes |
| `add_skill_evidence` | `skillId`, `title`, `evidenceType`, `level?`, `score?`, `sourceUrl?` | needs `domain:skills` writes; only a rubric level changes the skill level |

Errors are returned as tool results with `isError: true` (e.g. `permission denied: … outside the scope shared with MCP clients`).

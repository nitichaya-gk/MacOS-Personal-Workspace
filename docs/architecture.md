# Architecture

Follows PRD §11: a Tauri desktop shell, a React web UI, and a Rust data service on the device.

```
React UI (WKWebView)
   │  invoke("rpc", {method, params})    invoke("upload_asset", bytes)    wsasset://localhost/<id>
   ▼
commands/         thin Tauri commands; one Mutex<Workspace> per app
   ▼
application/      use cases: pages, databases, tasks, canvases, templates, export, settings, mcp_access
   ▼                    ▲
domain/           pure rules (validation, conversion, filter/sort, text & link extraction)
   ▼
infrastructure/   SQLite (rusqlite, bundled), filesystem, FTS5 search, backup/restore, CSV

integrations/notifications   background thread: reminders, daily summary, missed-reminder catch-up, auto-backup
integrations/mcp             stdio JSON-RPC server → same application/ services, read-only, scoped, audited
```

## Decisions

- **One RPC entry point.** The UI calls `rpc(method, params)`; `commands/mod.rs` maps methods to application functions. MCP calls the same functions, so validation and side effects (search index, links, reminder rescheduling) are identical (PRD §11.2, §17.1).
- **Pages are the tree.** Plain pages, projects, databases, database records and canvases are all rows in `pages` with a `kind`. Trash, archive, favourites, recent, backlinks, search and templates therefore work the same for every kind. A record shares its id with its page.
- **Blocks are rows.** BlockNote's document is flattened into `blocks(page_id, id, parent_block_id, position, …)` on save and rebuilt on load. `text` holds extracted plain text for search and Markdown.
- **Links by id.** Mentions, page-link blocks, embeds, task links and canvas object links are extracted into `links(source, target)` on save. Renaming or moving never breaks them.
- **Search is derived.** `search_docs` + FTS5 (`tokenize='trigram'`) is maintained after each save and can be rebuilt. Trigram gives substring matching for Thai, which has no spaces between words; terms under 3 characters fall back to `LIKE`.
- **Durability.** WAL with `synchronous=FULL`, transactions for multi-row changes, atomic temp-file + rename for every file written, `VACUUM INTO` for consistent backups while the app runs, a snapshot before any migration.
- **Time.** `due_date` is a local calendar date; `remind_at` is a UTC instant. Changing time zone changes what "Today" is, never when a reminder fires. Reminders more than 2 minutes late (app closed, Mac asleep) are marked missed and announced once.
- **Notification click.** The desktop notification plugin has no click callback, so the app remembers the last fired reminder and opens that task when the window gains focus within 3 minutes — which is what clicking the notification does.
- **Canvas engine.** Excalidraw (MIT) provides drawing, binding arrows, grouping, alignment, layers, locking, undo and export. Elements are stored one row per object in `canvas_objects` as editable JSON; images go to `assets/`. Fonts are bundled for offline use.
- **Assets.** Files are stored as `assets/<uuid>.<ext>` and served through the `wsasset://` protocol, so URLs stay valid when the workspace moves or is restored elsewhere. Asset ids are validated against path traversal.

## Personal Growth decisions (answers to the extension PRD's open ADRs)

- **Typed join tables, not a generic `entity_links`.** `task_requirement_links`, `task_skill_links`, `goal_skills`, `task_dependencies` keep SQLite foreign keys and cascades. The existing `links` table still handles free-form references.
- **One task, many goals, no double counting.** A task's allocations across requirements must total ≤ 100%, and goal metrics collect each task once.
- **Requirement progress.** Binary-style types count 0 until met; numeric/score use `clamp(current ÷ target)`; missing values are *unknown*, reported separately. Measured requirements can be marked met below target only with an override reason.
- **Skill levels.** Only from evidence with a rubric level; the rubric is versioned so old evidence stays tied to the rubric it was scored with. Default rubric is generic (0–5) and editable per skill.
- **Health** is rules-based (blocked requirement, missed deadlines, time used vs preparation, overdue tasks, unverified/outdated criteria (> 90 days), no review for 14 days) and is never presented as a probability.
- **Money** is stored as integer minor units; totals are per currency with no conversion; transfers between different currencies are refused.
- **MCP writes** go through drafts only. When the app is closed, the MCP process still writes drafts through the same services to the same SQLite file (WAL), and the user applies them later.
- **Paused goals** do not stop linked habit reminders automatically (left to the user).
- **Nested transactions.** `infrastructure::database::begin` joins an outer transaction when one is open, so applying a draft (requirements → milestones → tasks) is atomic.

## Known limits

- Reminders run in the app while it is open. When it is closed they need the optional background agent (Settings → General → Notifications), a per-user LaunchAgent that shares a claim table with the app so nothing is notified twice.
- CSP is disabled (`csp: null`); the app loads no remote content, but a strict CSP should be added after testing Excalidraw/Mermaid under it.
- Database queries load one database into memory for filtering/sorting; at 100k records a query takes ~0.3 s (see [performance.md](performance.md)).

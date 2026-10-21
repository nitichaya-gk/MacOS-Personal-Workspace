# Data model (schema v1)

Source of truth: [`src-tauri/migrations/0001_init.sql`](../src-tauri/migrations/0001_init.sql). Version is tracked with `PRAGMA user_version`; migrations run in a transaction after a `VACUUM INTO backups/pre-migration-v<N>-<time>.sqlite` copy.

| Table | Purpose |
| --- | --- |
| `workspaces` | One row: id, name, created_at |
| `settings` | JSON values: language, timezone, dailySummary, backup, mcp, plus internal bookkeeping |
| `sections` | Sidebar groups; `kind` projects / knowledge / databases / templates / custom |
| `pages` | Tree of everything: `kind` page / project / database / record / canvas; `parent_id` (cascade), `section_id`, `favorite`, `is_template`, `archived_at`, `deleted_at` |
| `blocks` | Flattened BlockNote blocks per page: `(page_id, id)` key, parent block, position, type, props/content JSON, extracted text |
| `links` | Internal references by id (source page/task/canvas → target) with context snippet; powers backlinks |
| `databases` | Database settings (`mcp_access` inherit / deny) |
| `database_properties` | name, type, JSON config (select options with id/name/color, status groups) |
| `database_records` | Record ↔ database membership and manual order; id = the record's page id |
| `record_values` | JSON value per (record, property); computed types are not stored |
| `database_views` | table / board / calendar / list with JSON config `{filter, sorts, hidden, groupBy, dateProperty}` |
| `canvases` | diagram / wireframe + saved view state |
| `canvas_objects` | One editable element JSON per object |
| `tasks` | title, description, status, priority, due_date (local date), remind_at (UTC), project, source page, completed/deleted |
| `reminders` | pending / fired / cancelled / missed; at most one pending per task |
| `assets` | Attachment metadata; file lives in `assets/<id>` |
| `mcp_audit_logs` | client, tool, target, result, time (90-day retention) |
| `search_docs` + `search_fts` | Derived full-text index (rebuildable) |
| `templates`, `forms`, `automations`, `automation_runs`, `versions` | Reserved for phase 3 |

## Value formats

| Property type | Stored JSON |
| --- | --- |
| text, url, email | `"string"` |
| number | `12.5` |
| select, status | `"<option id>"` |
| multi_select | `["<option id>", …]` |
| date | `{"start": "YYYY-MM-DD", "end"?: "YYYY-MM-DD"}` |
| checkbox | `true` (false = no row) |
| files | `[{"assetId": "…", "name": "…"}]` |
| title, created_time, last_edited_time | computed from `pages` |

## Export formats

- Page → Markdown (GFM), attachments copied to `<name>_assets/`; warnings list anything simplified (toggles, embeds).
- Database → CSV (UTF-8 with BOM, Thai headers preserved) or JSON (`format: personal-workspace-database`).
- Canvas → PNG, SVG, or `.excalidraw` JSON.
- Workspace → zip of `workspace.sqlite` + `assets/` + `manifest.json` (`format: personal-workspace`).

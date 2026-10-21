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
| `templates`, `forms`, `automations`, `automation_runs`, `versions` | Templates (phase 1); the others are filled in by schema v3 |

### Schema v2 — Personal Growth (`0002_growth.sql`)

| Table | Purpose |
| --- | --- |
| `goals` | domain, status, dates, priority, time budget, success definition, risk, last reviewed, `is_example`, `version` (optimistic concurrency) |
| `goal_requirements` | type, critical, weight, target/current value, unit, deadline, official source + checked date, verification and satisfaction status, override reason |
| `milestones` | goal, optional requirement, phase, due date, status, acceptance criteria, weight |
| `tasks` (+columns) | `milestone_id`, `estimated_effort_minutes`, `actual_effort_minutes` |
| `task_requirement_links` | task ↔ requirement with `allocation_pct` (per task total ≤ 100%) and rationale |
| `task_dependencies` | task depends on task; cycles rejected in the service |
| `skills`, `goal_skills`, `task_skill_links` | skills with a 6-level rubric and `rubric_version`; required level per goal; practised skills per task |
| `skill_evidence` | type, level (nullable), score/max, rubric version at the time, task/requirement/page/URL provenance, observed date, reviewer |
| `focus_sessions` | started/ended, minutes, quality, energy, note; an unfinished row is the running timer |
| `habits`, `habit_logs` | schedule JSON; one log per (habit, date) with done/missed/skipped |
| `financial_accounts`, `financial_categories`, `transactions` | integer minor units; transfers are a signed pair sharing `transfer_group_id` |
| `budgets`, `savings_goals`, `savings_allocations` | monthly limit per category+currency; allocations earmark existing cash |
| `reviews` | weekly summary snapshot + reflection, learnings, next commitments |
| `drafts` | pending AI proposals (goal breakdowns) |
| `audit_events` | finance create/update/delete/restore/allocate history |

Migration v1 → v2 only adds tables/columns. Existing pages and tasks are kept as they are; no goals, skills or evidence are inferred (tested in `tests/growth.rs`).

### Schema v3 — phase 3 (`0003_phase3.sql`)

| Table | Purpose |
| --- | --- |
| `database_views` (rebuilt) | adds `timeline`, `gallery`, `chart` view types |
| `forms` | named form per database: which properties, required flags, thank-you text |
| `automations`, `automation_runs` | trigger + conditions + actions JSON; runs keyed by `idempotency_key` so an event never fires twice |
| `notification_outbox` | notifications produced by automations, delivered by the app or the background agent |
| `versions` (+`source`, `title`) | page snapshots: manual, automatic, before an MCP edit, before a restore |
| `comments` | private per-page comments |

Relation, rollup and formula properties keep their configuration on the property. Relation values are `["<record id>", …]`; rollups and formulas are computed on read and never stored.

### Schema v4 — timetable (`0004_timetable.sql`)

| Table | Purpose |
| --- | --- |
| `time_blocks` | title, kind, weekday or one-off date, start/end minutes, repeat range, linked goal/task, reminder offset, per-occurrence exceptions |

### Schema v5 — growth phases B–F (`0005_growth2.sql`)

| Table | Purpose |
| --- | --- |
| `learning_courses`, `learning_topics`, `learning_resources` | courses → topics (prerequisites JSON, SM-2 `ease` / `interval_days` / `review_due`, `error_count`) → resources |
| `focus_sessions` (+`topic_id`) | focus time can count toward a topic |
| `recurring_transactions` | weekly/monthly/yearly bills and income with `next_date`; month-end dates clamp |
| `tasks` (+`source_canvas_id`, `source_element_id`) | the canvas object a task was created from |
| `drafts` (rebuilt) | `kind` is `goal_breakdown` or `transaction` |

Encrypted backups (`.pwbackup`) wrap the normal backup zip as `PWENC1` + salt (16 bytes) + nonce (12 bytes) + AES-256-GCM ciphertext, with the key from PBKDF2-HMAC-SHA256(password, salt, 600,000 rounds).

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

# Personal Workspace (macOS)

A local-first workspace for one person on one Mac: pages with a block editor, databases (Table / Board / Calendar / List), tasks with a Today view and macOS reminders, diagram and wireframe canvases, templates, full-text search (Thai and English), backup/restore, export, and a read-only MCP server for Claude and Codex. Built from [PRD.md](PRD.md) (roadmap phases 1 and 2) plus the Personal Growth extension PRD (goals, learning, habits, finance — the §21 MVP vertical slice).

Everything is stored on this Mac in a folder you control; nothing is sent anywhere unless you export it or connect an MCP client.

## Run it

Requirements: macOS 13+, Xcode Command Line Tools, Node 20+, Rust (installed with `rustup`).

```sh
npm install
npm run app:dev      # development app with hot reload
npm run app:build    # release .app and .dmg in src-tauri/target/release/bundle/
npm test             # Rust unit + acceptance tests (PRD §16, extension §16/§20)
```

The release app is ad-hoc signed. On first launch from Finder, macOS may ask you to confirm opening it (right-click → Open).

## Where data lives

```
~/Library/Application Support/com.personalworkspace.desktop/
├── config.json              # which workspace folder is open
└── Workspace/               # default workspace (you can create/open others in Settings → Workspace)
    ├── workspace.sqlite     # all structured data (WAL, synchronous=FULL)
    ├── assets/              # attachments, referenced by asset id
    ├── previews/            # regenerable derived files
    ├── backups/             # snapshots: <timestamp>/{workspace.sqlite, assets/, manifest.json}
    └── manifest.json        # format + schema version + workspace id
```

## What is in phase 1 + 2

| Area | Included |
| --- | --- |
| Workspace | Sections (Projects, Knowledge Base, Databases, Templates + your own), nested pages, drag-and-drop with cycle prevention, Favorites, Recent, Archive, Trash with restore |
| Editor | BlockNote: text styles, H1–H3, lists, to-dos, toggles, quote, callout, divider, tables, code, images/video/audio/files, Mermaid (errors shown, source kept), subpages, page links, `@` mentions, inline database views, canvas embeds, task blocks; `/` menu, drag handles, undo/redo, autosave with status |
| Databases | 13 property types, nested AND/OR filters, multi-level sort, hidden properties per view, Table/Board/Calendar/List views, record pages, record templates, type change with data-loss preview, CSV import/export, JSON export |
| Tasks | Status, priority, due date, reminder, project, links; Today/Overdue/Upcoming/No date/Completed; to-do → task conversion; one pending reminder per task, missed reminders summarised once, daily summary, time-zone aware |
| Canvas | Excalidraw engine (pan, zoom, grid, snap, shapes, arrows that follow, text, images, groups, align, distribute, layers, lock, duplicate, undo); diagram shape kit and sticky notes; wireframe frames (Desktop/Tablet/Mobile) and UI kit; links to pages; PNG/SVG/`.excalidraw` export of all or selection |
| Search | SQLite FTS5 trigram index → substring search that works for Thai; filters by type, project, edit date |
| Data safety | Atomic exports, `VACUUM INTO` backups with assets, daily auto-backup + retention, zip export, restore into a new folder with integrity report, pre-migration snapshot |
| MCP | stdio server inside the app binary (`--mcp`), off by default, scoped to chosen sections/pages, per-database opt-out, audit log |
| UI | Light/Dark (follows macOS), Thai/English UI, ⌘K command palette, keyboard shortcuts, inspector panel |

### Personal Growth extension

| Area | Included |
| --- | --- |
| Goals & roadmap | Goals by domain (education, career, finance, personal, project) with measurable success definition; weighted requirements (binary, numeric, score, document, portfolio, external verification) with official source, check date and verified/unverified/outdated state; phase → milestone → task roadmap; task dependencies with cycle protection; tasks link to several requirements with an allocation that can never exceed 100% |
| Progress | Four separate dimensions — task completion, roadmap progress, requirement preparation, skill readiness — each with its formula one click away; unknown progress shown as "unknown", never as 0; rules-based health (on track / at risk / blocked / needs review) with reasons; a goal cannot be marked achieved while a critical requirement is unmet or success is undefined |
| Learning & skills | Skills with a per-skill, versioned 0–5 rubric; evidence (mock test, assessment, code review, project, certificate…) with provenance; levels change only from assessed evidence, never from finished tasks; confidence and staleness; goal skill gaps |
| Focus & habits | Persistent focus timer that rolls minutes into task effort; habits (daily, chosen weekdays, N× per week) with done / skipped / missed, streaks that ignore rest days, 7-day consistency, daily reminders |
| Finance | Manual accounts, income/expense/transfer stored in integer satang, transfers never counted as income or expense, per-category monthly budgets, savings goals whose allocations cannot exceed available cash, links to goals, soft delete + audit trail, amounts hidden by default |
| Reviews & Today | Home shows goal pulse, focus timer, habits due and a collapsed budget pulse; weekly review drafted from local data (tasks done/slipped, milestones, evidence, habits, focus vs plan, spending, progress delta) that you edit and save |
| AI (MCP) | `list_goals`, `get_goal_roadmap`, `get_goal_health`, `get_skill_gap`, `get_today_plan`, `get_habit_summary`, `get_finance_summary` (only with an explicit finance grant), and `suggest_goal_breakdown`, which saves a draft you review and apply in the app |

The bundled CEDT CU goal is an illustrative scenario: its criteria are placeholders marked unverified, not real admission requirements.

Not yet: timeline/Gantt and split view, canvas-selection → task, spaced repetition, CSV import for transactions, recurring transactions, encrypted backups, automations, MCP tools that write directly.

Not yet from the first PRD (phase 3+): formulas/relations/rollups, timeline/gallery/chart views, forms, automations, MCP write tools, version history, comments, static site export.

## Connect Claude or Codex (MCP)

Enable it in **Settings → MCP**, choose what to share, then copy the snippet shown there. For example:

```sh
claude mcp add personal-workspace -- "/Applications/Personal Workspace.app/Contents/MacOS/personal-workspace" --mcp
```

Tools: `search_workspace`, `list_projects`, `read_page`, `query_database`, `list_tasks`, `read_canvas`, plus the growth tools above. See [docs/mcp-tools.md](docs/mcp-tools.md).

## Project layout

```
src/                       React + TypeScript UI
  app/                     App shell, routes, providers (store, i18n, Thai strings)
  components/{layout,ui}/  Sidebar, top bar, command palette, popovers, modals
  features/                pages, block-editor, databases, diagrams, wireframes, tasks, projects,
                           templates, search, settings, workspace (trash/archive/sections)
  services/desktop-bridge/ The only module that calls Rust (typed `rpc` wrapper)
  shared/                  Types and utilities
src-tauri/
  migrations/              Versioned SQL schema (PRAGMA user_version)
  src/domain/              Rules: property validation/conversion, filter/sort engine, text & link extraction
  src/application/         Services shared by UI and MCP: pages, databases, tasks, canvases, templates, settings, export
  src/infrastructure/      SQLite, filesystem, search index, backup/restore, CSV
  src/integrations/        Reminder scheduler (notifications), MCP stdio server
  src/commands/            Tauri commands (`rpc` dispatcher, raw asset upload, print, open)
  tests/acceptance.rs      Integration tests mapped to PRD acceptance criteria
docs/                      Architecture, data model, MCP tools
```
# MacOS-Personal-Workspace

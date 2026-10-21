# Personal Workspace (macOS)

A local-first workspace for one person on one Mac: pages with a block editor, databases with formulas, relations and seven view types, tasks with a Today view and macOS reminders, a weekly timetable, diagram and wireframe canvases, forms, automations, version history, full-text search (Thai and English), backup/restore (optionally encrypted), export, and an MCP server for Claude and Codex. Built from [PRD.md](PRD.md) (all four roadmap phases) plus the Personal Growth extension PRD (goals, learning, habits, finance — phases A–F).

Everything is stored on this Mac in a folder you control; nothing is sent anywhere unless you export it or connect an MCP client.

## Run it

Requirements: macOS 13+, Xcode Command Line Tools, Node 20+, Rust (installed with `rustup`).

```sh
npm install
npm run app:dev      # development app with hot reload
npm run app:build    # release .app in src-tauri/target/release/bundle/macos/
npm run app:dmg      # .dmg installer from that .app
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

## What is included

### Core workspace (PRD phases 1–4)

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
| UI | Light/Dark (follows macOS), Thai/English UI, ⌘K command palette, keyboard shortcuts, inspector panel, split view (⌥-click a page, or ⋯ → Open in split view) |
| Databases+ (phase 3) | Relations (two-way titles), rollups, a safe formula language (`prop("Price") * 1.07`, `if`, `dateBetween`, …) with cycle detection; Timeline, Gallery and Chart views; forms that add records |
| Timetable | Weekly day × time grid of recurring or one-off blocks (class, study, work, exercise…), per-occurrence edits, reminders, today's schedule on Home, planned vs actual focus |
| Automations | When a record/task is created or changes (or on a schedule) → set a value, create a task, notify; idempotent runs with a history |
| History & comments | Page version history (manual, automatic, before MCP edits) with restore; private comments per page |
| Publishing | Export selected pages as a static website (HTML + assets) |
| Background agent (phase 4) | Optional LaunchAgent (`--agent`) that delivers reminders, the daily summary, habit and timetable reminders and scheduled automations while the app is closed, without double-notifying |
| Canvas → task | Turn sticky notes or labelled shapes into tasks; the object links to the task and the task opens the canvas at that object |

### Personal Growth extension

| Area | Included |
| --- | --- |
| Goals & roadmap | Goals by domain (education, career, finance, personal, project) with measurable success definition; weighted requirements (binary, numeric, score, document, portfolio, external verification) with official source, check date and verified/unverified/outdated state; phase → milestone → task roadmap; task dependencies with cycle protection; tasks link to several requirements with an allocation that can never exceed 100% |
| Progress | Four separate dimensions — task completion, roadmap progress, requirement preparation, skill readiness — each with its formula one click away; unknown progress shown as "unknown", never as 0; rules-based health (on track / at risk / blocked / needs review) with reasons; a goal cannot be marked achieved while a critical requirement is unmet or success is undefined |
| Learning & skills | Skills with a per-skill, versioned 0–5 rubric; evidence (mock test, assessment, code review, project, certificate…) with provenance; levels change only from assessed evidence, never from finished tasks; confidence and staleness; goal skill gaps |
| Focus & habits | Persistent focus timer that rolls minutes into task effort; habits (daily, chosen weekdays, N× per week) with done / skipped / missed, streaks that ignore rest days, 7-day consistency, daily reminders |
| Finance | Manual accounts, income/expense/transfer stored in integer satang, transfers never counted as income or expense, per-category monthly budgets, savings goals whose allocations cannot exceed available cash, links to goals, soft delete + audit trail, amounts hidden by default |
| Learning hub | Courses → topics with prerequisites (cycle-checked), resources, spaced-repetition reviews (SM-2: Forgot / Hard / Good / Easy), weak-spot list; topics can link to skills but reviews never change skill levels |
| Finance+ | Recurring bills/income you confirm or skip when due; CSV import with column mapping, dry-run preview, Buddhist-era dates and duplicate skipping; AI-suggested entries wait as drafts until you pick an account |
| Planning aids | Goal Timeline tab (milestones as diamonds, tasks by due date, drag to reschedule, dependency arrows); "Loose ends" check for goals without criteria, unplanned requirements, unlinked tasks and skills without evidence |
| Reviews & Today | Home shows goal pulse, focus timer, habits due and a collapsed budget pulse; weekly review drafted from local data (tasks done/slipped, milestones, evidence, habits, focus vs plan, spending, progress delta) that you edit and save |
| AI (MCP) | Growth read tools (goals, roadmap, health, skill gap, today plan, timetable, learning plan, habits, weekly review summary; finance only with an explicit grant); `suggest_goal_breakdown` and `create_transaction_draft` save drafts you apply in the app; logging focus, habits and skill evidence needs a write grant |
| Data safety+ | Encrypted backups (`.pwbackup`, AES-256-GCM, key from your password via PBKDF2) in Settings → Backup |

The bundled CEDT CU goal is an illustrative scenario: its criteria are placeholders marked unverified, not real admission requirements.

Not included, by design (local-first, single user): bank connections, sync, collaboration, cloud hosting. Performance measurements are in [docs/performance.md](docs/performance.md).

## Connect Claude or Codex (MCP)

Enable it in **Settings → MCP**, choose what to share, then copy the snippet shown there. For example:

```sh
claude mcp add personal-workspace -- "/Applications/Personal Workspace.app/Contents/MacOS/personal-workspace" --mcp
```

Reading is scoped to what you share; writing (pages, records, tasks, growth logs) is a separate opt-in per section, project or domain, and page edits keep a revision. See [docs/mcp-tools.md](docs/mcp-tools.md).

## Project layout

```
src/                       React + TypeScript UI
  app/                     App shell, routes, providers (store, i18n, Thai strings)
  components/{layout,ui}/  Sidebar, top bar, command palette, popovers, modals
  features/                pages, block-editor, databases, diagrams, wireframes, tasks, projects, timetable,
                           automations, goals, learning, skills, habits, finance, review,
                           templates, search, settings, workspace (trash/archive/sections)
  services/desktop-bridge/ The only module that calls Rust (typed `rpc` wrapper)
  shared/                  Types and utilities
src-tauri/
  migrations/              Versioned SQL schema (PRAGMA user_version)
  src/domain/              Rules: property validation/conversion, filter/sort engine, formulas, growth metrics
  src/application/         Services shared by UI and MCP (pages, databases, tasks, goals, learning, finance, …)
  src/infrastructure/      SQLite, filesystem, search index, backup/restore/encryption, CSV, Markdown
  src/integrations/        Reminder scheduler, background agent, shared digest, MCP stdio server
  src/commands/            Tauri commands (`rpc` dispatcher, raw asset upload, print, open)
  tests/                   Integration tests mapped to PRD acceptance criteria (acceptance, growth, phase3, …)
  examples/                Demo workspace, HTTP bridge for browser testing, benchmark
docs/                      Architecture, data model, MCP tools
```

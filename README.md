# Personal Workspace (macOS)

A local-first workspace for one person on one Mac: pages with a block editor, databases (Table / Board / Calendar / List), tasks with a Today view and macOS reminders, diagram and wireframe canvases, templates, full-text search (Thai and English), backup/restore, export, and a read-only MCP server for Claude and Codex. Built from [PRD.md](PRD.md) — roadmap phases 1 and 2.

Everything is stored on this Mac in a folder you control; nothing is sent anywhere unless you export it or connect an MCP client.

## Run it

Requirements: macOS 13+, Xcode Command Line Tools, Node 20+, Rust (installed with `rustup`).

```sh
npm install
npm run app:dev      # development app with hot reload
npm run app:build    # release .app and .dmg in src-tauri/target/release/bundle/
npm test             # Rust unit + acceptance tests (PRD §16)
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

Not yet (phase 3+): formulas/relations/rollups, timeline/gallery/chart views, forms, automations, MCP write tools, version history, comments, static site export.

## Connect Claude or Codex (MCP)

Enable it in **Settings → MCP**, choose what to share, then copy the snippet shown there. For example:

```sh
claude mcp add personal-workspace -- "/Applications/Personal Workspace.app/Contents/MacOS/personal-workspace" --mcp
```

Tools: `search_workspace`, `list_projects`, `read_page`, `query_database`, `list_tasks`, `read_canvas`. See [docs/mcp-tools.md](docs/mcp-tools.md).

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

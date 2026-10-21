-- Schema version 1. Every entity uses a stable TEXT (UUID) id; names are never identifiers.

CREATE TABLE workspaces (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  created_at  TEXT NOT NULL
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL -- JSON
);

CREATE TABLE sections (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  icon        TEXT,
  kind        TEXT NOT NULL CHECK (kind IN ('projects','knowledge','databases','templates','custom')),
  position    REAL NOT NULL,
  created_at  TEXT NOT NULL
);

-- Pages are the unit of the tree: plain pages, projects, databases, database records and canvases.
CREATE TABLE pages (
  id              TEXT PRIMARY KEY,
  section_id      TEXT REFERENCES sections(id) ON DELETE SET NULL,
  parent_id       TEXT REFERENCES pages(id) ON DELETE CASCADE,
  kind            TEXT NOT NULL CHECK (kind IN ('page','project','database','record','canvas')),
  title           TEXT NOT NULL DEFAULT '',
  icon            TEXT,
  position        REAL NOT NULL DEFAULT 0,
  is_template     INTEGER NOT NULL DEFAULT 0,
  favorite        INTEGER NOT NULL DEFAULT 0,
  archived_at     TEXT,
  deleted_at      TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  last_opened_at  TEXT
);
CREATE INDEX idx_pages_parent ON pages(parent_id, position);
CREATE INDEX idx_pages_section ON pages(section_id, position);
CREATE INDEX idx_pages_opened ON pages(last_opened_at);

CREATE TABLE blocks (
  page_id          TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
  id               TEXT NOT NULL,
  parent_block_id  TEXT,
  position         INTEGER NOT NULL,
  type             TEXT NOT NULL,
  props            TEXT NOT NULL DEFAULT '{}',
  content          TEXT NOT NULL DEFAULT '[]',
  text             TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (page_id, id)
);
CREATE INDEX idx_blocks_order ON blocks(page_id, parent_block_id, position);

-- Internal links (page mentions, embeds, task links, canvas object links) by id.
CREATE TABLE links (
  source_id    TEXT NOT NULL,
  source_type  TEXT NOT NULL CHECK (source_type IN ('page','task','canvas')),
  target_id    TEXT NOT NULL,
  context      TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (source_id, target_id)
);
CREATE INDEX idx_links_target ON links(target_id);

CREATE TABLE databases (
  id          TEXT PRIMARY KEY REFERENCES pages(id) ON DELETE CASCADE,
  mcp_access  TEXT NOT NULL DEFAULT 'inherit' CHECK (mcp_access IN ('inherit','deny'))
);

CREATE TABLE database_properties (
  id           TEXT PRIMARY KEY,
  database_id  TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  type         TEXT NOT NULL,
  config       TEXT NOT NULL DEFAULT '{}',
  position     REAL NOT NULL
);
CREATE INDEX idx_props_db ON database_properties(database_id, position);

-- A record shares its id with its page, so it is the same id in every view.
CREATE TABLE database_records (
  id           TEXT PRIMARY KEY REFERENCES pages(id) ON DELETE CASCADE,
  database_id  TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
  position     REAL NOT NULL
);
CREATE INDEX idx_records_db ON database_records(database_id, position);

CREATE TABLE record_values (
  record_id    TEXT NOT NULL REFERENCES database_records(id) ON DELETE CASCADE,
  property_id  TEXT NOT NULL REFERENCES database_properties(id) ON DELETE CASCADE,
  value        TEXT NOT NULL, -- JSON
  PRIMARY KEY (record_id, property_id)
);
CREATE INDEX idx_values_prop ON record_values(property_id);

CREATE TABLE database_views (
  id           TEXT PRIMARY KEY,
  database_id  TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  type         TEXT NOT NULL CHECK (type IN ('table','board','calendar','list')),
  config       TEXT NOT NULL DEFAULT '{}',
  position     REAL NOT NULL
);

CREATE TABLE canvases (
  id          TEXT PRIMARY KEY REFERENCES pages(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL CHECK (kind IN ('diagram','wireframe')),
  app_state   TEXT NOT NULL DEFAULT '{}',
  updated_at  TEXT NOT NULL
);

CREATE TABLE canvas_objects (
  canvas_id  TEXT NOT NULL REFERENCES canvases(id) ON DELETE CASCADE,
  id         TEXT NOT NULL,
  position   INTEGER NOT NULL,
  type       TEXT NOT NULL,
  data       TEXT NOT NULL, -- editable element JSON, never a flattened image
  PRIMARY KEY (canvas_id, id)
);

CREATE TABLE tasks (
  id               TEXT PRIMARY KEY,
  title            TEXT NOT NULL,
  description      TEXT NOT NULL DEFAULT '',
  status           TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo','in_progress','done')),
  priority         TEXT NOT NULL DEFAULT 'none' CHECK (priority IN ('none','low','medium','high','urgent')),
  due_date         TEXT,      -- local calendar date YYYY-MM-DD in the user's time zone
  remind_at        TEXT,      -- UTC instant
  project_id       TEXT REFERENCES pages(id) ON DELETE SET NULL,
  source_page_id   TEXT REFERENCES pages(id) ON DELETE SET NULL,
  completed_at     TEXT,
  deleted_at       TEXT,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);
CREATE INDEX idx_tasks_due ON tasks(status, due_date);
CREATE INDEX idx_tasks_project ON tasks(project_id);

CREATE TABLE reminders (
  id          TEXT PRIMARY KEY,
  task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  fire_at     TEXT NOT NULL,
  status      TEXT NOT NULL CHECK (status IN ('pending','fired','cancelled','missed')),
  fired_at    TEXT,
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_reminders_pending ON reminders(status, fire_at);

CREATE TABLE assets (
  id          TEXT PRIMARY KEY,   -- file name inside assets/ (uuid + extension)
  file_name   TEXT NOT NULL,      -- original name, may be Thai
  mime        TEXT NOT NULL,
  size        INTEGER NOT NULL,
  page_id     TEXT REFERENCES pages(id) ON DELETE SET NULL,
  created_at  TEXT NOT NULL
);

-- Reserved for later phases (forms, automations, versions); created now so later migrations stay additive.
CREATE TABLE templates (
  id          TEXT PRIMARY KEY,
  page_id     TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
  created_at  TEXT NOT NULL
);
CREATE TABLE forms (id TEXT PRIMARY KEY, database_id TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE, config TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL);
CREATE TABLE automations (id TEXT PRIMARY KEY, name TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 0, config TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL);
CREATE TABLE automation_runs (id TEXT PRIMARY KEY, automation_id TEXT NOT NULL REFERENCES automations(id) ON DELETE CASCADE, status TEXT NOT NULL, detail TEXT, at TEXT NOT NULL);
CREATE TABLE versions (id TEXT PRIMARY KEY, page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE, snapshot TEXT NOT NULL, created_at TEXT NOT NULL);

CREATE TABLE mcp_audit_logs (
  id       TEXT PRIMARY KEY,
  client   TEXT NOT NULL,
  tool     TEXT NOT NULL,
  target   TEXT,
  result   TEXT NOT NULL,   -- ok | denied | error
  detail   TEXT,
  at       TEXT NOT NULL
);
CREATE INDEX idx_mcp_audit_at ON mcp_audit_logs(at);

-- Search index (derived data; can be rebuilt). Trigram tokenizer gives substring search for Thai, which has no word spaces.
CREATE TABLE search_docs (
  rowid        INTEGER PRIMARY KEY,
  entity_id    TEXT NOT NULL UNIQUE,
  entity_type  TEXT NOT NULL,
  project_id   TEXT,
  updated_at   TEXT NOT NULL,
  title        TEXT NOT NULL,
  body         TEXT NOT NULL
);
CREATE VIRTUAL TABLE search_fts USING fts5(title, body, content='search_docs', content_rowid='rowid', tokenize='trigram');
CREATE TRIGGER search_docs_ai AFTER INSERT ON search_docs BEGIN
  INSERT INTO search_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
END;
CREATE TRIGGER search_docs_ad AFTER DELETE ON search_docs BEGIN
  INSERT INTO search_fts(search_fts, rowid, title, body) VALUES ('delete', old.rowid, old.title, old.body);
END;
CREATE TRIGGER search_docs_au AFTER UPDATE ON search_docs BEGIN
  INSERT INTO search_fts(search_fts, rowid, title, body) VALUES ('delete', old.rowid, old.title, old.body);
  INSERT INTO search_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
END;

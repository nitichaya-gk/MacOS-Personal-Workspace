-- Schema version 3: advanced database views, forms/automation bookkeeping, page comments, version metadata.

-- Database views gain timeline, gallery and chart (CHECK constraints can only change by rebuilding the table).
CREATE TABLE database_views_v3 (
  id           TEXT PRIMARY KEY,
  database_id  TEXT NOT NULL REFERENCES databases(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  type         TEXT NOT NULL CHECK (type IN ('table','board','calendar','list','timeline','gallery','chart')),
  config       TEXT NOT NULL DEFAULT '{}',
  position     REAL NOT NULL
);
INSERT INTO database_views_v3 (id, database_id, name, type, config, position) SELECT id, database_id, name, type, config, position FROM database_views;
DROP TABLE database_views;
ALTER TABLE database_views_v3 RENAME TO database_views;
CREATE INDEX idx_views_db ON database_views(database_id, position);

ALTER TABLE forms ADD COLUMN name TEXT NOT NULL DEFAULT 'Form';
ALTER TABLE forms ADD COLUMN updated_at TEXT;

ALTER TABLE automations ADD COLUMN updated_at TEXT;
ALTER TABLE automation_runs ADD COLUMN idempotency_key TEXT;
ALTER TABLE automation_runs ADD COLUMN event_json TEXT;
CREATE UNIQUE INDEX idx_auto_idem ON automation_runs(automation_id, idempotency_key) WHERE idempotency_key IS NOT NULL;
CREATE INDEX idx_auto_runs_at ON automation_runs(automation_id, at);

-- Notifications requested by automations (or the MCP process); the app's scheduler delivers them.
CREATE TABLE notification_outbox (
  id          TEXT PRIMARY KEY,
  title       TEXT NOT NULL,
  body        TEXT NOT NULL DEFAULT '',
  target_id   TEXT,
  created_at  TEXT NOT NULL,
  sent_at     TEXT
);

-- Page revisions (versions table exists since v1).
ALTER TABLE versions ADD COLUMN source TEXT NOT NULL DEFAULT 'autosave';
ALTER TABLE versions ADD COLUMN title TEXT NOT NULL DEFAULT '';
CREATE INDEX idx_versions_page ON versions(page_id, created_at);

-- Private comments on pages, optionally anchored to a block.
CREATE TABLE comments (
  id           TEXT PRIMARY KEY,
  page_id      TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
  block_id     TEXT,
  quote        TEXT NOT NULL DEFAULT '',
  body         TEXT NOT NULL,
  resolved_at  TEXT,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);
CREATE INDEX idx_comments_page ON comments(page_id, created_at);

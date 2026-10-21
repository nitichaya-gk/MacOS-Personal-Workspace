-- Schema version 5: learning hub, recurring bills, canvas → task provenance, transaction drafts.

CREATE TABLE learning_courses (
  id          TEXT PRIMARY KEY,
  title       TEXT NOT NULL,
  provider    TEXT NOT NULL DEFAULT '',
  url         TEXT NOT NULL DEFAULT '',
  goal_id     TEXT REFERENCES goals(id) ON DELETE SET NULL,
  status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('planned','active','paused','done')),
  start_date  TEXT,
  end_date    TEXT,
  notes       TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  deleted_at  TEXT
);

CREATE TABLE learning_topics (
  id                TEXT PRIMARY KEY,
  course_id         TEXT REFERENCES learning_courses(id) ON DELETE CASCADE,
  title             TEXT NOT NULL,
  skill_id          TEXT REFERENCES skills(id) ON DELETE SET NULL,
  prerequisites     TEXT NOT NULL DEFAULT '[]',
  status            TEXT NOT NULL DEFAULT 'not_started' CHECK (status IN ('not_started','learning','reviewing','mastered')),
  position          REAL NOT NULL DEFAULT 0,
  ease              REAL NOT NULL DEFAULT 2.5,
  interval_days     INTEGER NOT NULL DEFAULT 0,
  review_due        TEXT,
  last_reviewed_at  TEXT,
  error_count       INTEGER NOT NULL DEFAULT 0,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL
);
CREATE INDEX idx_topics_due ON learning_topics(review_due);

CREATE TABLE learning_resources (
  id          TEXT PRIMARY KEY,
  course_id   TEXT REFERENCES learning_courses(id) ON DELETE CASCADE,
  topic_id    TEXT REFERENCES learning_topics(id) ON DELETE CASCADE,
  title       TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'link' CHECK (kind IN ('link','book','video','exercise','note','file')),
  url         TEXT NOT NULL DEFAULT '',
  page_id     TEXT REFERENCES pages(id) ON DELETE SET NULL,
  done        INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL
);

ALTER TABLE focus_sessions ADD COLUMN topic_id TEXT REFERENCES learning_topics(id) ON DELETE SET NULL;

CREATE TABLE recurring_transactions (
  id              TEXT PRIMARY KEY,
  title           TEXT NOT NULL,
  type            TEXT NOT NULL CHECK (type IN ('income','expense')),
  account_id      TEXT NOT NULL REFERENCES financial_accounts(id),
  category_id     TEXT REFERENCES financial_categories(id) ON DELETE SET NULL,
  amount_minor    INTEGER NOT NULL CHECK (amount_minor > 0),
  frequency       TEXT NOT NULL CHECK (frequency IN ('weekly','monthly','yearly')),
  next_date       TEXT NOT NULL,
  end_date        TEXT,
  linked_goal_id  TEXT REFERENCES goals(id) ON DELETE SET NULL,
  active          INTEGER NOT NULL DEFAULT 1,
  created_at      TEXT NOT NULL
);

ALTER TABLE tasks ADD COLUMN source_canvas_id TEXT REFERENCES pages(id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN source_element_id TEXT;

-- Drafts may now also hold transaction proposals (still nothing is saved until the user applies them).
CREATE TABLE drafts_v5 (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('goal_breakdown','transaction')),
  target_id    TEXT,
  payload_json TEXT NOT NULL,
  source       TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','applied','discarded')),
  created_at   TEXT NOT NULL,
  resolved_at  TEXT
);
INSERT INTO drafts_v5 SELECT id, kind, target_id, payload_json, source, status, created_at, resolved_at FROM drafts;
DROP TABLE drafts;
ALTER TABLE drafts_v5 RENAME TO drafts;

-- Schema version 2: Personal Growth layer (goals, roadmap, skills, habits, focus, finance, reviews, drafts).
-- Existing pages/tasks are preserved; no goals, skills or evidence are fabricated for old data.

-- ---------- goals & roadmap ----------
CREATE TABLE goals (
  id                          TEXT PRIMARY KEY,
  title                       TEXT NOT NULL,
  description                 TEXT NOT NULL DEFAULT '',
  domain                      TEXT NOT NULL CHECK (domain IN ('education','career','finance','personal','project')),
  motivation                  TEXT NOT NULL DEFAULT '',
  start_date                  TEXT,
  target_date                 TEXT,
  status                      TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','active','paused','achieved','cancelled')),
  priority                    INTEGER NOT NULL DEFAULT 2,     -- 1 high … 3 low
  time_budget_hours_per_week  REAL,
  budget_limit_minor          INTEGER,
  currency                    TEXT NOT NULL DEFAULT 'THB',
  success_definition          TEXT NOT NULL DEFAULT '',
  risk_level                  TEXT NOT NULL DEFAULT 'medium' CHECK (risk_level IN ('low','medium','high')),
  confidence                  TEXT NOT NULL DEFAULT 'unknown' CHECK (confidence IN ('unknown','low','medium','high')),
  notes                       TEXT NOT NULL DEFAULT '',
  project_id                  TEXT REFERENCES pages(id) ON DELETE SET NULL,
  last_reviewed_at            TEXT,
  is_example                  INTEGER NOT NULL DEFAULT 0,     -- illustrative scenario, criteria not verified
  created_at                  TEXT NOT NULL,
  updated_at                  TEXT NOT NULL,
  deleted_at                  TEXT,
  version                     INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE goal_requirements (
  id                      TEXT PRIMARY KEY,
  goal_id                 TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  title                   TEXT NOT NULL,
  description             TEXT NOT NULL DEFAULT '',
  type                    TEXT NOT NULL DEFAULT 'binary' CHECK (type IN ('binary','numeric','score','document','portfolio','external_verification')),
  critical                INTEGER NOT NULL DEFAULT 1,
  weight                  REAL NOT NULL DEFAULT 1 CHECK (weight >= 0),
  target_value            REAL,
  current_value           REAL,
  unit                    TEXT NOT NULL DEFAULT '',
  deadline                TEXT,
  verification_source     TEXT NOT NULL DEFAULT '',
  source_checked_at       TEXT,
  verification_status     TEXT NOT NULL DEFAULT 'unverified' CHECK (verification_status IN ('unverified','verified','outdated')),
  satisfaction_status     TEXT NOT NULL DEFAULT 'not_started' CHECK (satisfaction_status IN ('not_started','in_progress','met','not_met','blocked','unknown')),
  manual_override_reason  TEXT NOT NULL DEFAULT '',
  position                REAL NOT NULL DEFAULT 0,
  created_at              TEXT NOT NULL,
  updated_at              TEXT NOT NULL
);
CREATE INDEX idx_req_goal ON goal_requirements(goal_id, position);

CREATE TABLE milestones (
  id                   TEXT PRIMARY KEY,
  goal_id              TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  requirement_id       TEXT REFERENCES goal_requirements(id) ON DELETE SET NULL,
  phase                TEXT NOT NULL DEFAULT '',
  title                TEXT NOT NULL,
  due_date             TEXT,
  status               TEXT NOT NULL DEFAULT 'planned' CHECK (status IN ('planned','in_progress','met','missed')),
  acceptance_criteria  TEXT NOT NULL DEFAULT '',
  weight               REAL NOT NULL DEFAULT 1 CHECK (weight >= 0),
  position             REAL NOT NULL DEFAULT 0,
  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL
);
CREATE INDEX idx_ms_goal ON milestones(goal_id, position);

ALTER TABLE tasks ADD COLUMN milestone_id TEXT REFERENCES milestones(id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN estimated_effort_minutes INTEGER;
ALTER TABLE tasks ADD COLUMN actual_effort_minutes INTEGER;
CREATE INDEX idx_tasks_ms ON tasks(milestone_id);

-- Typed joins (ADR: typed tables for validated relations instead of a polymorphic entity_links table).
CREATE TABLE task_requirement_links (
  task_id         TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  requirement_id  TEXT NOT NULL REFERENCES goal_requirements(id) ON DELETE CASCADE,
  allocation_pct  REAL NOT NULL DEFAULT 100 CHECK (allocation_pct > 0 AND allocation_pct <= 100),
  rationale       TEXT NOT NULL DEFAULT '',
  created_at      TEXT NOT NULL,
  PRIMARY KEY (task_id, requirement_id)
);
CREATE INDEX idx_trl_req ON task_requirement_links(requirement_id);

CREATE TABLE task_dependencies (
  task_id        TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  depends_on_id  TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  created_at     TEXT NOT NULL,
  PRIMARY KEY (task_id, depends_on_id),
  CHECK (task_id <> depends_on_id)
);

-- ---------- skills & evidence ----------
CREATE TABLE skills (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL,
  category        TEXT NOT NULL DEFAULT '',
  description     TEXT NOT NULL DEFAULT '',
  target_level    INTEGER NOT NULL DEFAULT 3 CHECK (target_level BETWEEN 0 AND 5),
  rubric_json     TEXT NOT NULL DEFAULT '[]',   -- descriptions of levels 0..5 for this skill
  rubric_version  INTEGER NOT NULL DEFAULT 1,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  deleted_at      TEXT
);

CREATE TABLE goal_skills (
  goal_id       TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  skill_id      TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  target_level  INTEGER NOT NULL DEFAULT 3 CHECK (target_level BETWEEN 0 AND 5),
  PRIMARY KEY (goal_id, skill_id)
);

CREATE TABLE task_skill_links (
  task_id   TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  skill_id  TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, skill_id)
);

CREATE TABLE skill_evidence (
  id              TEXT PRIMARY KEY,
  skill_id        TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  evidence_type   TEXT NOT NULL CHECK (evidence_type IN ('assessment','mock_test','artifact','code_review','project','certificate','self_review')),
  title           TEXT NOT NULL,
  level           INTEGER CHECK (level BETWEEN 0 AND 5),  -- rubric level assessed (NULL = not assessed)
  score           REAL,
  max_score       REAL,
  rubric_version  INTEGER,
  task_id         TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  requirement_id  TEXT REFERENCES goal_requirements(id) ON DELETE SET NULL,
  source_item_id  TEXT REFERENCES pages(id) ON DELETE SET NULL,
  source_url      TEXT NOT NULL DEFAULT '',
  observed_at     TEXT NOT NULL,                       -- local date YYYY-MM-DD
  reviewer        TEXT NOT NULL DEFAULT '',
  notes           TEXT NOT NULL DEFAULT '',
  created_at      TEXT NOT NULL
);
CREATE INDEX idx_ev_skill ON skill_evidence(skill_id, observed_at);
CREATE INDEX idx_ev_req ON skill_evidence(requirement_id);

CREATE TABLE focus_sessions (
  id               TEXT PRIMARY KEY,
  task_id          TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  goal_id          TEXT REFERENCES goals(id) ON DELETE SET NULL,
  skill_id         TEXT REFERENCES skills(id) ON DELETE SET NULL,
  started_at       TEXT NOT NULL,
  ended_at         TEXT,
  planned_minutes  INTEGER,
  actual_minutes   INTEGER,
  focus_quality    INTEGER CHECK (focus_quality BETWEEN 1 AND 5),
  energy           INTEGER CHECK (energy BETWEEN 1 AND 5),
  note             TEXT NOT NULL DEFAULT '',
  created_at       TEXT NOT NULL
);
CREATE INDEX idx_focus_started ON focus_sessions(started_at);

-- ---------- habits ----------
CREATE TABLE habits (
  id             TEXT PRIMARY KEY,
  title          TEXT NOT NULL,
  description    TEXT NOT NULL DEFAULT '',
  kind           TEXT NOT NULL DEFAULT 'binary' CHECK (kind IN ('binary','count','duration','numeric')),
  target_value   REAL,
  unit           TEXT NOT NULL DEFAULT '',
  schedule_json  TEXT NOT NULL DEFAULT '{"type":"daily"}',  -- daily | weekdays{days:[1..7]} | weekly{perWeek}
  goal_id        TEXT REFERENCES goals(id) ON DELETE SET NULL,
  skill_id       TEXT REFERENCES skills(id) ON DELETE SET NULL,
  reminder_time  TEXT,
  start_date     TEXT NOT NULL,
  archived_at    TEXT,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);

CREATE TABLE habit_logs (
  id              TEXT PRIMARY KEY,
  habit_id        TEXT NOT NULL REFERENCES habits(id) ON DELETE CASCADE,
  scheduled_date  TEXT NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('done','missed','skipped')),
  value           REAL,
  note            TEXT NOT NULL DEFAULT '',
  created_at      TEXT NOT NULL,
  UNIQUE (habit_id, scheduled_date)
);

-- ---------- finance (amounts in integer minor units, e.g. satang) ----------
CREATE TABLE financial_accounts (
  id                     TEXT PRIMARY KEY,
  name                   TEXT NOT NULL,
  type                   TEXT NOT NULL DEFAULT 'cash' CHECK (type IN ('cash','bank','savings','credit','ewallet','other')),
  currency               TEXT NOT NULL DEFAULT 'THB',
  opening_balance_minor  INTEGER NOT NULL DEFAULT 0,
  archived_at            TEXT,
  created_at             TEXT NOT NULL
);

CREATE TABLE financial_categories (
  id        TEXT PRIMARY KEY,
  name      TEXT NOT NULL,
  kind      TEXT NOT NULL CHECK (kind IN ('income','expense')),
  color     TEXT NOT NULL DEFAULT 'gray',
  position  REAL NOT NULL DEFAULT 0
);

CREATE TABLE transactions (
  id                 TEXT PRIMARY KEY,
  account_id         TEXT NOT NULL REFERENCES financial_accounts(id),
  type               TEXT NOT NULL CHECK (type IN ('income','expense','transfer')),
  -- income/expense: positive; transfer: signed (negative leaves the account, positive enters it)
  amount_minor       INTEGER NOT NULL CHECK (amount_minor <> 0),
  currency           TEXT NOT NULL,
  category_id        TEXT REFERENCES financial_categories(id) ON DELETE SET NULL,
  occurred_on        TEXT NOT NULL,
  merchant_or_source TEXT NOT NULL DEFAULT '',
  note               TEXT NOT NULL DEFAULT '',
  linked_goal_id     TEXT REFERENCES goals(id) ON DELETE SET NULL,
  transfer_group_id  TEXT,
  created_at         TEXT NOT NULL,
  updated_at         TEXT NOT NULL,
  deleted_at         TEXT
);
CREATE INDEX idx_tx_date ON transactions(occurred_on);
CREATE INDEX idx_tx_account ON transactions(account_id);

CREATE TABLE budgets (
  id           TEXT PRIMARY KEY,
  category_id  TEXT NOT NULL REFERENCES financial_categories(id) ON DELETE CASCADE,
  limit_minor  INTEGER NOT NULL CHECK (limit_minor > 0),
  currency     TEXT NOT NULL DEFAULT 'THB',
  goal_id      TEXT REFERENCES goals(id) ON DELETE SET NULL,
  created_at   TEXT NOT NULL,
  UNIQUE (category_id, currency)
);

CREATE TABLE savings_goals (
  id            TEXT PRIMARY KEY,
  title         TEXT NOT NULL,
  target_minor  INTEGER NOT NULL CHECK (target_minor > 0),
  currency      TEXT NOT NULL DEFAULT 'THB',
  target_date   TEXT,
  goal_id       TEXT REFERENCES goals(id) ON DELETE SET NULL,
  created_at    TEXT NOT NULL,
  deleted_at    TEXT
);

CREATE TABLE savings_allocations (
  id               TEXT PRIMARY KEY,
  savings_goal_id  TEXT NOT NULL REFERENCES savings_goals(id) ON DELETE CASCADE,
  amount_minor     INTEGER NOT NULL CHECK (amount_minor <> 0),
  allocated_on     TEXT NOT NULL,
  note             TEXT NOT NULL DEFAULT '',
  created_at       TEXT NOT NULL
);

-- ---------- reviews, drafts, audit ----------
CREATE TABLE reviews (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL DEFAULT 'weekly' CHECK (kind IN ('weekly','monthly')),
  period_key  TEXT NOT NULL,          -- e.g. 2026-W43
  summary_json TEXT NOT NULL DEFAULT '{}',
  reflection  TEXT NOT NULL DEFAULT '',
  learnings   TEXT NOT NULL DEFAULT '',
  next_commitments TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  UNIQUE (kind, period_key)
);

-- Proposals from AI clients. Nothing here changes real data until the user applies it in the app.
CREATE TABLE drafts (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('goal_breakdown')),
  target_id   TEXT,
  payload_json TEXT NOT NULL,
  source      TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','applied','discarded')),
  created_at  TEXT NOT NULL,
  resolved_at TEXT
);

CREATE TABLE audit_events (
  id           TEXT PRIMARY KEY,
  entity_type  TEXT NOT NULL,
  entity_id    TEXT NOT NULL,
  action       TEXT NOT NULL,
  before_json  TEXT,
  after_json   TEXT,
  at           TEXT NOT NULL
);
CREATE INDEX idx_audit_entity ON audit_events(entity_type, entity_id);

INSERT INTO financial_categories(id, name, kind, color, position) VALUES
  ('cat-salary', 'Salary / Allowance', 'income', 'green', 0),
  ('cat-side', 'Side income', 'income', 'blue', 1),
  ('cat-food', 'Food', 'expense', 'orange', 10),
  ('cat-transport', 'Transport', 'expense', 'yellow', 11),
  ('cat-education', 'Education', 'expense', 'purple', 12),
  ('cat-equipment', 'Equipment', 'expense', 'blue', 13),
  ('cat-fees', 'Application fees', 'expense', 'pink', 14),
  ('cat-housing', 'Housing & bills', 'expense', 'brown', 15),
  ('cat-other', 'Other', 'expense', 'gray', 16);

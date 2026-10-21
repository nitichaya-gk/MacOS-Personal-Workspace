-- Schema version 4: timetable (planned time blocks per day and time).
CREATE TABLE time_blocks (
  id                 TEXT PRIMARY KEY,
  title              TEXT NOT NULL,
  kind               TEXT NOT NULL DEFAULT 'study' CHECK (kind IN ('study','work','class','habit','personal','other')),
  recurrence         TEXT NOT NULL DEFAULT 'once' CHECK (recurrence IN ('once','weekly')),
  date               TEXT,               -- for one-off blocks (YYYY-MM-DD, local)
  weekdays           TEXT NOT NULL DEFAULT '[]', -- weekly: [1..7], 1 = Monday
  start_time         TEXT NOT NULL,      -- HH:MM local
  end_time           TEXT NOT NULL,      -- HH:MM local, after start
  valid_from         TEXT,               -- weekly series range (e.g. a semester)
  valid_until        TEXT,
  skip_dates         TEXT NOT NULL DEFAULT '[]', -- occurrences removed from a weekly series
  task_id            TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  goal_id            TEXT REFERENCES goals(id) ON DELETE SET NULL,
  habit_id           TEXT REFERENCES habits(id) ON DELETE SET NULL,
  location           TEXT NOT NULL DEFAULT '',
  notes              TEXT NOT NULL DEFAULT '',
  remind_minutes     INTEGER,            -- notify this many minutes before start
  created_at         TEXT NOT NULL,
  updated_at         TEXT NOT NULL,
  deleted_at         TEXT
);
CREATE INDEX idx_blocks_date ON time_blocks(recurrence, date);

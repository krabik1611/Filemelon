ALTER TABLE rules ADD COLUMN min_age_seconds INTEGER NOT NULL DEFAULT 86400 CHECK (min_age_seconds >= 0);
CREATE TABLE rule_runs (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 rule_id INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
 rule_name TEXT NOT NULL,
 trigger TEXT NOT NULL CHECK (trigger IN ('manual','cron')),
 start_time TEXT NOT NULL,
 end_time TEXT,
 status TEXT NOT NULL CHECK (status IN ('running','success','partial','failed','interrupted')),
 processed INTEGER NOT NULL DEFAULT 0,
 skipped INTEGER NOT NULL DEFAULT 0,
 failed INTEGER NOT NULL DEFAULT 0,
 error TEXT
);
CREATE INDEX idx_rule_runs_rule_id ON rule_runs(rule_id);
CREATE INDEX idx_rule_runs_start_time ON rule_runs(start_time);

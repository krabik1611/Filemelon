ALTER TABLE rules ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
UPDATE rules SET sort_order = id;
CREATE INDEX IF NOT EXISTS idx_rules_sort_order ON rules(sort_order, id);

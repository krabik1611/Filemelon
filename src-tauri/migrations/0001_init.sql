CREATE TABLE rules (
                       id INTEGER PRIMARY KEY AUTOINCREMENT,
                       name TEXT NOT NULL,
                       source TEXT NOT NULL,
                       expr TEXT NOT NULL,
                       cron TEXT NOT NULL,
                       enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE rule_actions (
                              id INTEGER PRIMARY KEY AUTOINCREMENT,
                              rule_id INTEGER NOT NULL,

                              action_type INTEGER NOT NULL,
                              destination_template TEXT,

                              FOREIGN KEY (rule_id)
                                  REFERENCES rules(id)
                                  ON DELETE CASCADE
);
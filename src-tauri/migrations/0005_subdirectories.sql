ALTER TABLE rules ADD COLUMN include_subdirectories INTEGER NOT NULL DEFAULT 0 CHECK (include_subdirectories IN (0, 1));

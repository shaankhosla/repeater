-- Create the version update table.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS version_update (
    last_prompted_at TEXT,
    last_version_check_at TEXT
) STRICT;


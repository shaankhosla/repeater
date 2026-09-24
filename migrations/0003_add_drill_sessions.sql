-- Persist agent-driven drill sessions and individual review attempts.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS drill_sessions (
    session_id TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('paths', 'apple_notes')),
    source_paths_json TEXT NOT NULL,
    retention REAL NOT NULL CHECK (retention >= 0.65 AND retention <= 1.0),
    rephrase_questions INTEGER NOT NULL CHECK (rephrase_questions IN (0, 1)),
    shuffle INTEGER NOT NULL CHECK (shuffle IN (0, 1)),
    state TEXT NOT NULL CHECK (state IN ('active', 'complete', 'expired')),
    created_at TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    completed_at TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS drill_reviews (
    review_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    card_hash TEXT NOT NULL,
    card_kind TEXT NOT NULL CHECK (card_kind IN ('basic', 'cloze')),
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    source_path TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('presented', 'revealed', 'marked')),
    review_result TEXT CHECK (review_result IS NULL OR review_result IN ('pass', 'fail')),
    presented_at TEXT NOT NULL,
    revealed_at TEXT,
    marked_at TEXT,
    interval_raw REAL,
    interval_days INTEGER,
    due_at TEXT,
    resulting_review_count INTEGER,
    FOREIGN KEY (session_id) REFERENCES drill_sessions(session_id) ON DELETE CASCADE,
    FOREIGN KEY (card_hash) REFERENCES cards(card_hash)
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_drill_reviews_one_open_per_session
ON drill_reviews(session_id)
WHERE state IN ('presented', 'revealed');

CREATE INDEX IF NOT EXISTS idx_drill_reviews_session_history
ON drill_reviews(session_id, presented_at);

CREATE INDEX IF NOT EXISTS idx_drill_sessions_expiration
ON drill_sessions(expires_at);

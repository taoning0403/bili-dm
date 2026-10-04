CREATE TABLE playback_progress (
    path TEXT PRIMARY KEY REFERENCES media(path) ON DELETE CASCADE,
    position REAL NOT NULL CHECK (position >= 0),
    duration REAL NOT NULL CHECK (duration > 0),
    completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE playback_preferences (id INTEGER PRIMARY KEY CHECK (id = 1), json TEXT NOT NULL);
PRAGMA user_version = 2;

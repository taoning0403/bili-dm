CREATE TABLE torrent_tasks (
    torrent_id TEXT PRIMARY KEY NOT NULL,
    magnet_uri TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    status TEXT NOT NULL CHECK (status IN ('ready', 'downloading', 'paused')),
    name TEXT NOT NULL,
    file_count INTEGER NOT NULL,
    catalog_json TEXT NOT NULL
);

CREATE TABLE media (
    id TEXT PRIMARY KEY NOT NULL,
    path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    duration REAL CHECK (duration IS NULL OR duration > 0),
    torrent_id TEXT REFERENCES torrent_tasks(torrent_id),
    file_index INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX torrent_tasks_recent ON torrent_tasks(updated_at DESC);
CREATE INDEX media_torrent ON media(torrent_id);
PRAGMA user_version = 1;

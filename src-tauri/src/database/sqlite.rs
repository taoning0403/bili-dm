use super::{
    models::{MediaRecord, PlaybackPreferences, PlaybackProgress, TorrentRecord},
    LibraryRepository,
};
use crate::{
    core::error::{AppError, AppResult},
    torrent::models::TorrentCatalog,
};
use async_trait::async_trait;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct SqliteLibrary {
    connection: Arc<Mutex<Connection>>,
}

fn database_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("DATABASE", format!("本地数据库操作失败：{error}"))
}

impl SqliteLibrary {
    /// Startup-only operation, before the window begins accepting commands.
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(AppError::io)?;
        }
        let mut connection = Connection::open(path).map_err(database_error)?;
        connection
            .busy_timeout(Duration::from_secs(3))
            .map_err(database_error)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .map_err(database_error)?;
        let version: u32 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(database_error)?;
        if version > 2 {
            return Err(AppError::new(
                "DATABASE_VERSION",
                "数据库来自更新版本，请使用新版应用打开。",
            ));
        }
        if version == 0 {
            let transaction = connection.transaction().map_err(database_error)?;
            transaction
                .execute_batch(include_str!("migrations/001_library.sql"))
                .map_err(database_error)?;
            transaction.commit().map_err(database_error)?;
        }
        if version < 2 {
            let transaction = connection.transaction().map_err(database_error)?;
            transaction
                .execute_batch(include_str!("migrations/002_playback.sql"))
                .map_err(database_error)?;
            transaction.commit().map_err(database_error)?;
        }
        // Restoring history must never automatically restart network activity.
        connection
            .execute(
                "UPDATE torrent_tasks SET status = 'paused' WHERE status = 'downloading'",
                [],
            )
            .map_err(database_error)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
    ) -> AppResult<T> {
        let connection = self.connection.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = connection
                .lock()
                .map_err(|_| database_error("连接锁不可用"))?;
            work(&mut guard)
        })
        .await
        .map_err(database_error)?
    }
}

#[async_trait]
impl LibraryRepository for SqliteLibrary {
    async fn save_catalog(&self, magnet: &str, catalog: &TorrentCatalog) -> AppResult<()> {
        let magnet = magnet.trim().to_owned();
        let catalog = catalog.clone();
        let json = serde_json::to_string(&catalog).map_err(database_error)?;
        let file_count = u32::try_from(catalog.files.len()).map_err(database_error)?;
        self.run(move |connection| {
            connection.execute(
                "INSERT INTO torrent_tasks (torrent_id, magnet_uri, status, name, file_count, catalog_json)
                 VALUES (?1, ?2, 'ready', ?3, ?4, ?5)
                 ON CONFLICT(torrent_id) DO UPDATE SET magnet_uri = excluded.magnet_uri,
                 name = excluded.name, file_count = excluded.file_count, catalog_json = excluded.catalog_json, updated_at = unixepoch()",
                params![catalog.id, magnet, catalog.name, file_count, json],
            ).map_err(database_error)?;
            Ok(())
        }).await
    }

    async fn recent_torrents(&self) -> AppResult<Vec<TorrentRecord>> {
        self.run(|connection| {
            let mut statement = connection.prepare(
                "SELECT torrent_id, magnet_uri, name, created_at, status, file_count FROM torrent_tasks ORDER BY updated_at DESC, torrent_id LIMIT 20"
            ).map_err(database_error)?;
            let rows = statement.query_map([], |row| Ok(TorrentRecord {
                torrent_id: row.get(0)?, magnet_uri: row.get(1)?, name: row.get(2)?,
                created_at: row.get(3)?, status: row.get(4)?, file_count: row.get(5)?,
            })).map_err(database_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(database_error)
        }).await
    }

    async fn save_media(&self, media: MediaRecord) -> AppResult<()> {
        let file_index = media
            .file_index
            .map(i64::try_from)
            .transpose()
            .map_err(database_error)?;
        if media
            .duration
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(database_error("媒体时长必须是有限正数"));
        }
        self.run(move |connection| {
            let transaction = connection.transaction().map_err(database_error)?;
            transaction.execute(
                "INSERT INTO media (id, path, filename, duration, torrent_id, file_index) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET duration = COALESCE(excluded.duration, media.duration), updated_at = unixepoch()",
                params![media.id, media.path, media.filename, media.duration, media.torrent_id, file_index],
            ).map_err(database_error)?;
            if let Some(id) = media.torrent_id {
                transaction.execute("UPDATE torrent_tasks SET status = 'downloading', updated_at = unixepoch() WHERE torrent_id = ?1", [id]).map_err(database_error)?;
            }
            transaction.commit().map_err(database_error)
        }).await
    }

    async fn pause_task(&self, torrent_id: &str) -> AppResult<()> {
        let id = torrent_id.to_owned();
        self.run(move |connection| {
            connection.execute("UPDATE torrent_tasks SET status = 'paused', updated_at = unixepoch() WHERE torrent_id = ?1", [id]).map_err(database_error)?;
            Ok(())
        }).await
    }

    async fn progress(&self, path: &str) -> AppResult<Option<PlaybackProgress>> {
        let path = path.to_owned();
        self.run(move |connection| {
            connection.query_row("SELECT path, position, duration, completed FROM playback_progress WHERE path=?1",[path],|row|Ok(PlaybackProgress {
                path:row.get(0)?,position:row.get(1)?,duration:row.get(2)?,completed:row.get(3)?,
            })).optional().map_err(database_error)
        }).await
    }
    async fn save_progress(&self, progress: PlaybackProgress) -> AppResult<()> {
        if !progress.position.is_finite()
            || !progress.duration.is_finite()
            || progress.duration <= 0.0
            || progress.position < 0.0
            || progress.position > progress.duration + 1.0
        {
            return Err(database_error("无效的播放进度"));
        }
        self.run(move |connection| {
            connection.execute("INSERT INTO playback_progress (path,position,duration,completed) VALUES (?1,?2,?3,?4)
                ON CONFLICT(path) DO UPDATE SET position=excluded.position,duration=excluded.duration,completed=excluded.completed,updated_at=unixepoch()",
                params![progress.path,progress.position.min(progress.duration),progress.duration,progress.completed]).map_err(database_error)?;
            Ok(())
        }).await
    }
    async fn preferences(&self) -> AppResult<PlaybackPreferences> {
        self.run(|connection| {
            let json: Option<String> = connection
                .query_row(
                    "SELECT json FROM playback_preferences WHERE id=1",
                    [],
                    |r| r.get(0),
                )
                .optional()
                .map_err(database_error)?;
            json.map(|j| serde_json::from_str(&j).map_err(database_error))
                .transpose()
                .map(Option::unwrap_or_default)
        })
        .await
    }
    async fn save_preferences(&self, preferences: PlaybackPreferences) -> AppResult<()> {
        let json = serde_json::to_string(&preferences).map_err(database_error)?;
        self.run(move |connection| {
            connection.execute("INSERT INTO playback_preferences (id,json) VALUES (1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",[json]).map_err(database_error)?;
            Ok(())
        }).await
    }
}

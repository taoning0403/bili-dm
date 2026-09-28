use bili_dm_lib::{
    database::{models::MediaRecord, sqlite::SqliteLibrary, LibraryRepository},
    media::files::MediaFile,
    torrent::models::TorrentCatalog,
};
use rusqlite::Connection;

fn catalog() -> TorrentCatalog {
    TorrentCatalog {
        id: "test-hash".into(),
        name: "movie".into(),
        files: vec![MediaFile::new(3, "movie.mkv".into(), 1000)],
        suggested_file_index: Some(3),
    }
}

fn media(duration: Option<f64>) -> MediaRecord {
    MediaRecord {
        id: "torrent:test-hash:3".into(),
        path: "torrent://test-hash/3".into(),
        filename: "movie.mkv".into(),
        duration,
        torrent_id: Some("test-hash".into()),
        file_index: Some(3),
    }
}

#[tokio::test]
async fn persists_history_and_duration_without_auto_resuming(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let library = SqliteLibrary::open(&path)?;
    library.save_catalog("magnet:first", &catalog()).await?;
    let created_at = library.recent_torrents().await?[0].created_at;
    library.save_media(media(None)).await?;
    library.save_media(media(Some(120.5))).await?;
    library.save_media(media(None)).await?;
    library.save_catalog("magnet:updated", &catalog()).await?;
    assert_eq!(library.recent_torrents().await?[0].status, "downloading");
    drop(library);

    let restored = SqliteLibrary::open(&path)?;
    let tasks = restored.recent_torrents().await?;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, "paused");
    assert_eq!(tasks[0].magnet_uri, "magnet:updated");
    assert_eq!(tasks[0].created_at, created_at);
    let connection = Connection::open(&path)?;
    let (count, duration, index): (i64, f64, i64) = connection.query_row(
        "SELECT count(*), duration, file_index FROM media",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!((count, duration, index), (1, 120.5, 3));
    let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(version, 1);
    Ok(())
}

#[tokio::test]
async fn rejects_orphan_media_and_invalid_duration_atomically(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let library = SqliteLibrary::open(&path)?;
    assert!(library.save_media(media(None)).await.is_err());
    library.save_catalog("magnet:test", &catalog()).await?;
    assert!(library.save_media(media(Some(f64::NAN))).await.is_err());
    assert!(library.save_media(media(Some(-1.0))).await.is_err());
    let connection = Connection::open(path)?;
    let count: i64 = connection.query_row("SELECT count(*) FROM media", [], |row| row.get(0))?;
    assert_eq!(count, 0);
    assert_eq!(library.recent_torrents().await?[0].status, "ready");
    Ok(())
}

#[test]
fn refuses_a_newer_schema() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    Connection::open(&path)?.execute_batch("PRAGMA user_version = 99;")?;
    assert!(SqliteLibrary::open(&path).is_err());
    Ok(())
}

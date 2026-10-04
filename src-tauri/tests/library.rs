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
    assert_eq!(version, 2);
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

#[tokio::test]
async fn migrates_v1_in_place_and_validates_resume_positions(
) -> Result<(), Box<dyn std::error::Error>> {
    use bili_dm_lib::database::models::PlaybackProgress;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("legacy.sqlite3");
    let connection = Connection::open(&path)?;
    connection.execute_batch(include_str!("../src/database/migrations/001_library.sql"))?;
    connection.execute(
        "INSERT INTO torrent_tasks(torrent_id,magnet_uri,status,name,file_count,catalog_json) VALUES ('test-hash','magnet:legacy','downloading','movie',1,?1)",
        [serde_json::to_string(&catalog())?],
    )?;
    connection.execute(
        "INSERT INTO media(id,path,filename,duration,torrent_id,file_index) VALUES ('legacy-media','torrent://test-hash/3','movie.mkv',120,'test-hash',3)",
        [],
    )?;
    drop(connection);
    let library = SqliteLibrary::open(&path)?;
    let tasks = library.recent_torrents().await?;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].magnet_uri, "magnet:legacy");
    assert_eq!(tasks[0].status, "paused");
    let connection = Connection::open(&path)?;
    let (id, duration): (String, f64) = connection.query_row(
        "SELECT id,duration FROM media WHERE path='torrent://test-hash/3'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    assert_eq!(id, "legacy-media");
    assert_eq!(duration, 120.0);
    library
        .save_progress(PlaybackProgress {
            path: "torrent://test-hash/3".into(),
            position: 43.5,
            duration: 120.0,
            completed: false,
        })
        .await?;
    assert_eq!(
        library
            .progress("torrent://test-hash/3")
            .await?
            .ok_or("no progress")?
            .position,
        43.5
    );
    assert!(library
        .save_progress(PlaybackProgress {
            path: "torrent://test-hash/3".into(),
            position: f64::NAN,
            duration: 120.0,
            completed: false
        })
        .await
        .is_err());
    assert!(library
        .save_progress(PlaybackProgress {
            path: "torrent://test-hash/3".into(),
            position: 999.0,
            duration: 120.0,
            completed: false
        })
        .await
        .is_err());
    assert_eq!(
        library
            .progress("torrent://test-hash/3")
            .await?
            .ok_or("no progress")?
            .position,
        43.5
    );
    Ok(())
}

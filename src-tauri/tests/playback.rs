use async_trait::async_trait;
use bili_dm_lib::{
    core::{
        error::{AppError, AppResult},
        playback_service::{preferred_track, PlaybackService, QueueOptions},
    },
    database::{
        models::{RepeatMode, TrackPreference},
        sqlite::SqliteLibrary,
        LibraryRepository,
    },
    media::files::MediaFile,
    player::{MediaTrack, PlayerBackend, PlayerControl, PlayerSnapshot, VideoViewport},
    torrent::{
        models::{DownloadStats, PrefetchStats, TorrentCatalog},
        SeekableReader, TorrentEngine,
    },
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{Mutex, Notify};

#[derive(Default)]
struct Engine {
    active: Mutex<Option<DownloadStats>>,
    waiting: AtomicBool,
    entered: Notify,
}
fn catalog() -> TorrentCatalog {
    TorrentCatalog {
        id: "fixture".into(),
        name: "season".into(),
        suggested_file_index: Some(7),
        files: vec![
            MediaFile::new(2, "E10.mp4".into(), 100),
            MediaFile::new(7, "E01.mp4".into(), 100),
            MediaFile::new(4, "E02.mp4".into(), 100),
        ],
    }
}
#[async_trait]
impl TorrentEngine for Engine {
    async fn resolve(&self, _: &str) -> AppResult<TorrentCatalog> {
        Ok(catalog())
    }
    async fn import_torrent(&self, _: Vec<u8>) -> AppResult<TorrentCatalog> {
        Ok(catalog())
    }
    async fn catalog(&self, _: &str) -> AppResult<TorrentCatalog> {
        Ok(catalog())
    }
    async fn cancel_resolution(&self) {}
    async fn prepare_file(&self, id: &str, index: usize) -> AppResult<MediaFile> {
        *self.active.lock().await = Some(DownloadStats {
            torrent_id: id.into(),
            file_index: index,
            total: 100,
            downloaded: 10,
            selected_files: vec![index],
            ..Default::default()
        });
        self.entered.notify_one();
        if self.waiting.load(Ordering::Relaxed) {
            std::future::pending::<()>().await;
        }
        catalog()
            .files
            .into_iter()
            .find(|f| f.index == index)
            .ok_or_else(|| AppError::new("TEST", "bad index"))
    }
    async fn open_file(&self, _: &str, _: usize) -> AppResult<Box<dyn SeekableReader>> {
        Ok(Box::new(std::io::Cursor::new(vec![0; 100])))
    }
    async fn download_stats(&self) -> Option<DownloadStats> {
        self.active.lock().await.clone()
    }
    async fn prefetch(&self, _: &str, index: Option<usize>) -> AppResult<()> {
        if let Some(stats) = self.active.lock().await.as_mut() {
            stats.selected_files = vec![stats.file_index];
            stats.prefetch = index.map(|i| {
                stats.selected_files.push(i);
                PrefetchStats {
                    file_index: i,
                    total: 100,
                    downloaded: 0,
                }
            });
        }
        Ok(())
    }
    async fn subtitle_path(&self, _: &str, _: usize) -> AppResult<Option<PathBuf>> {
        Ok(None)
    }
    async fn pause_download(&self) -> AppResult<()> {
        self.active.lock().await.take();
        Ok(())
    }
    async fn shutdown(&self) {
        self.active.lock().await.take();
    }
}
#[derive(Default)]
struct Player {
    snapshot: Mutex<PlayerSnapshot>,
    fail: AtomicBool,
    loads: Mutex<Vec<f64>>,
}
#[async_trait]
impl PlayerBackend for Player {
    async fn load(&self, _: &str, start: f64) -> AppResult<()> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(AppError::new("TEST", "decoder failed"));
        }
        self.loads.lock().await.push(start);
        *self.snapshot.lock().await = PlayerSnapshot {
            running: true,
            loaded: true,
            position: start,
            duration: 120.0,
            volume: 100.0,
            speed: 1.0,
            ..Default::default()
        };
        Ok(())
    }
    async fn add_subtitle(&self, _: &str, _: &str) -> AppResult<()> {
        Ok(())
    }
    async fn viewport(&self, _: VideoViewport) -> AppResult<()> {
        Ok(())
    }
    async fn control(&self, control: PlayerControl) -> AppResult<()> {
        let mut p = self.snapshot.lock().await;
        match control {
            PlayerControl::Stop => *p = PlayerSnapshot::default(),
            PlayerControl::Pause { paused } => p.paused = paused,
            PlayerControl::Seek { seconds } => {
                p.position = seconds;
                p.ended = false;
            }
            PlayerControl::Speed { speed } => p.speed = speed,
            PlayerControl::Mute { muted } => p.muted = muted,
            PlayerControl::Volume { volume } => p.volume = volume,
            _ => {}
        }
        Ok(())
    }
    async fn snapshot(&self) -> AppResult<PlayerSnapshot> {
        Ok(self.snapshot.lock().await.clone())
    }
    async fn shutdown(&self) {
        *self.snapshot.lock().await = PlayerSnapshot::default();
    }
}
async fn setup() -> Result<
    (
        tempfile::TempDir,
        Arc<Engine>,
        Arc<Player>,
        Arc<SqliteLibrary>,
        Arc<PlaybackService>,
    ),
    Box<dyn std::error::Error>,
> {
    let dir = tempfile::tempdir()?;
    let library = Arc::new(SqliteLibrary::open(&dir.path().join("db"))?);
    library.save_catalog("magnet:test", &catalog()).await?;
    let engine = Arc::new(Engine::default());
    let player = Arc::new(Player::default());
    let service = Arc::new(PlaybackService::new(
        engine.clone(),
        player.clone(),
        library.clone(),
    ));
    Ok((dir, engine, player, library, service))
}
#[tokio::test]
async fn eof_advances_in_natural_order_and_completed_episode_restarts(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_dir, engine, player, library, service) = setup().await?;
    service.play_torrent("fixture", 7).await?;
    service.tick().await?;
    assert_eq!(
        service
            .snapshot()
            .await?
            .queue
            .iter()
            .map(|f| f.index)
            .collect::<Vec<_>>(),
        vec![7, 4, 2]
    );
    assert!(engine
        .download_stats()
        .await
        .ok_or("no active")?
        .prefetch
        .is_none());
    engine
        .active
        .lock()
        .await
        .as_mut()
        .ok_or("no active")?
        .downloaded = 100;
    service.tick().await?;
    assert_eq!(
        engine
            .download_stats()
            .await
            .ok_or("no active")?
            .prefetch
            .map(|p| p.file_index),
        Some(4)
    );
    {
        let mut p = player.snapshot.lock().await;
        p.ended = true;
        p.position = 120.0;
    }
    service.tick().await?;
    assert_eq!(
        service
            .snapshot()
            .await?
            .media
            .ok_or("no media")?
            .file_index,
        Some(4)
    );
    assert!(
        library
            .progress("torrent://fixture/7")
            .await?
            .ok_or("no progress")?
            .completed
    );
    let reopened = service.play_torrent("fixture", 7).await?;
    assert_eq!(reopened.resumed_from, 0.0);
    service.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn stop_saves_position_and_reopen_restores_without_eager_network(
) -> Result<(), Box<dyn std::error::Error>> {
    let (dir, engine, player, library, service) = setup().await?;
    service.play_torrent("fixture", 7).await?;
    service.tick().await?;
    player.snapshot.lock().await.position = 48.25;
    service
        .control(PlayerControl::Volume { volume: 35.0 })
        .await?;
    service.control(PlayerControl::Stop).await?;
    assert!(engine.download_stats().await.is_none());
    let reopened = SqliteLibrary::open(&dir.path().join("db"))?;
    assert_eq!(reopened.preferences().await?.volume, 35.0);
    assert_eq!(
        reopened
            .progress("torrent://fixture/7")
            .await?
            .ok_or("missing progress")?
            .position,
        48.25
    );
    assert_eq!(reopened.recent_torrents().await?[0].status, "paused");
    let resumed = service.play_torrent("fixture", 7).await?;
    assert_eq!(resumed.resumed_from, 48.25);
    assert_eq!(
        library
            .progress("torrent://fixture/7")
            .await?
            .ok_or("missing progress")?
            .position,
        48.25
    );
    service.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn cancel_open_and_decoder_failure_release_download() -> Result<(), Box<dyn std::error::Error>>
{
    let (_dir, engine, player, _library, service) = setup().await?;
    engine.waiting.store(true, Ordering::Relaxed);
    let pending = {
        let service = service.clone();
        tokio::spawn(async move { service.play_torrent("fixture", 7).await })
    };
    engine.entered.notified().await;
    // A published snapshot must not wait for metadata/cache/player opening.
    tokio::time::timeout(Duration::from_millis(100), service.snapshot()).await??;
    tokio::time::timeout(Duration::from_secs(1), service.control(PlayerControl::Stop)).await??;
    assert_eq!(
        pending.await?.err().ok_or("not cancelled")?.code,
        "CANCELLED"
    );
    assert!(engine.download_stats().await.is_none());
    engine.waiting.store(false, Ordering::Relaxed);
    player.fail.store(true, Ordering::Relaxed);
    assert!(service.play_torrent("fixture", 7).await.is_err());
    assert!(engine.download_stats().await.is_none());
    assert!(service.snapshot().await?.media.is_none());
    service.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn disabling_prefetch_removes_next_selection_and_auto_next_can_be_disabled(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_dir, engine, player, _library, service) = setup().await?;
    service.play_torrent("fixture", 7).await?;
    service.tick().await?;
    engine
        .active
        .lock()
        .await
        .as_mut()
        .ok_or("no active")?
        .downloaded = 100;
    service.tick().await?;
    service
        .queue_options(QueueOptions {
            auto_next: true,
            prefetch_next: true,
            repeat: RepeatMode::One,
        })
        .await?;
    service.tick().await?;
    assert_eq!(
        engine
            .download_stats()
            .await
            .ok_or("no active")?
            .selected_files,
        vec![7]
    );
    service
        .queue_options(QueueOptions {
            auto_next: false,
            prefetch_next: false,
            repeat: RepeatMode::Off,
        })
        .await?;
    assert_eq!(
        engine
            .download_stats()
            .await
            .ok_or("no active")?
            .selected_files,
        vec![7]
    );
    {
        let mut p = player.snapshot.lock().await;
        p.ended = true;
        p.position = 120.0;
    }
    service.tick().await?;
    assert_eq!(
        service
            .snapshot()
            .await?
            .media
            .ok_or("no media")?
            .file_index,
        Some(7)
    );
    assert_eq!(player.loads.lock().await.len(), 1);
    service.shutdown().await;
    Ok(())
}
#[test]
fn audio_preference_matches_language_instead_of_previous_track_id() {
    let tracks = vec![
        MediaTrack {
            id: 1,
            kind: "audio".into(),
            language: "jpn".into(),
            codec: "aac".into(),
            ..Default::default()
        },
        MediaTrack {
            id: 6,
            kind: "audio".into(),
            language: "eng".into(),
            codec: "aac".into(),
            ..Default::default()
        },
    ];
    let choice = TrackPreference {
        disabled: false,
        language: "eng".into(),
        title: String::new(),
        codec: "aac".into(),
    };
    assert_eq!(preferred_track(&tracks, "audio", &choice), Some(6));
}

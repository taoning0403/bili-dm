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

struct DanmakuProviderFixture {
    entered: Notify,
}
#[async_trait]
impl bili_dm_lib::danmaku::bilibili::DanmakuProvider for DanmakuProviderFixture {
    async fn search(
        &self,
        query: &str,
        _: u32,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> AppResult<bili_dm_lib::bilibili::discovery::SearchPage> {
        if query == "wait" {
            self.entered.notify_one();
            cancel.cancelled().await;
            return Err(bili_dm_lib::danmaku::bilibili::cancelled());
        }
        Ok(bili_dm_lib::bilibili::discovery::SearchPage {
            hits: if query == "limited" {
                vec!["bad", "bad", "short"]
            } else {
                vec!["short"]
            }
            .into_iter()
            .map(|input| bili_dm_lib::bilibili::discovery::SearchHit {
                input: input.into(),
                title: input.into(),
                kind: "video".into(),
            })
            .collect(),
            has_more: false,
            warnings: vec![],
        })
    }
    async fn videos(
        &self,
        input: &str,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> AppResult<Vec<bili_dm_lib::danmaku::models::SourceInfo>> {
        if input == "wait" {
            self.entered.notify_one();
            cancel.cancelled().await;
            return Err(bili_dm_lib::danmaku::bilibili::cancelled());
        }
        if input == "bad" {
            return Err(AppError::new("FIXTURE", "missing video"));
        }
        Ok(vec![bili_dm_lib::danmaku::models::SourceInfo {
            id: input.into(),
            bvid: "BV1xx411c7mD".into(),
            cid: 1,
            page: 1,
            title: input.into(),
            duration: 10.0,
            comment_count: 0,
            warnings: vec![],
            episode_id: None,
        }])
    }
    async fn comments(
        &self,
        mut info: bili_dm_lib::danmaku::models::SourceInfo,
        _: &tokio_util::sync::CancellationToken,
    ) -> AppResult<bili_dm_lib::danmaku::models::ParsedSource> {
        info.comment_count = 1;
        Ok(bili_dm_lib::danmaku::models::ParsedSource {
            info,
            comments: vec![bili_dm_lib::danmaku::models::Comment {
                id: "1".into(),
                time: 2.0,
                mode: 1,
                size: 25,
                color: 0xffffff,
                text: "hello".into(),
            }],
        })
    }
    async fn stream(
        &self,
        _: &bili_dm_lib::danmaku::models::SourceInfo,
        _: &tokio_util::sync::CancellationToken,
    ) -> AppResult<bili_dm_lib::danmaku::bilibili::VideoStream> {
        Err(AppError::new("FIXTURE", "not used"))
    }
}

#[tokio::test]
async fn smart_search_filters_short_candidates_before_decoding_and_cancels_on_stop(
) -> Result<(), Box<dyn std::error::Error>> {
    use bili_dm_lib::danmaku::{service::DanmakuService, strategy::MatchOptions};
    let (dir, _engine, _player, _library, playback) = setup().await?;
    let media = playback.play_torrent("fixture", 7).await?;
    playback.tick().await?;
    let provider = Arc::new(DanmakuProviderFixture {
        entered: Notify::new(),
    });
    // This directory deliberately has no decoder runtime: filtered candidates
    // must never reach expensive decoding or comment fetching.
    let service = Arc::new(DanmakuService::new(
        playback.clone(),
        provider.clone(),
        dir.path().into(),
        dir.path().join("dm"),
    ));
    let result = service
        .search_and_match(
            &media.session_id,
            "short".into(),
            1,
            MatchOptions::default(),
        )
        .await?;
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].state, "filtered");
    assert!(result.workspace.sources.is_empty());
    assert!(result.clips.is_empty());
    let limited = service
        .search_and_match(
            &media.session_id,
            "limited".into(),
            1,
            MatchOptions {
                max_candidates: 1,
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(limited.candidates.len(), 1);
    assert_eq!(limited.candidates[0].state, "unavailable");
    assert!(limited.errors.iter().any(|e| e.contains("1 个候选")));
    let task = {
        let service = service.clone();
        let session = media.session_id;
        tokio::spawn(async move {
            service
                .search_and_match(&session, "wait".into(), 1, MatchOptions::default())
                .await
        })
    };
    provider.entered.notified().await;
    playback.control(PlayerControl::Stop).await?;
    assert_eq!(task.await?.err().map(|e| e.code), Some("CANCELLED"));
    assert!(!service.status().running);
    Ok(())
}

#[tokio::test]
async fn danmaku_projects_persist_mixes_handle_partial_failures_and_reject_stale_sessions(
) -> Result<(), Box<dyn std::error::Error>> {
    use bili_dm_lib::danmaku::{models::Clip, service::DanmakuService};
    let (dir, _engine, _player, _library, playback) = setup().await?;
    let media = playback.play_torrent("fixture", 7).await?;
    playback.tick().await?;
    let provider = Arc::new(DanmakuProviderFixture {
        entered: Notify::new(),
    });
    let service = DanmakuService::new(
        playback.clone(),
        provider.clone(),
        dir.path().into(),
        dir.path().join("dm"),
    );
    let parsed = service
        .resolve(
            &media.session_id,
            vec!["a".into(), "bad".into(), "b".into()],
        )
        .await?;
    assert_eq!(parsed.sources.len(), 2);
    assert_eq!(parsed.errors.len(), 1);
    let clips = parsed
        .sources
        .iter()
        .map(|source| Clip {
            id: source.id.clone(),
            source_id: source.id.clone(),
            source_start: 0.0,
            source_end: 10.0,
            target_start: 20.0,
            target_end: 30.0,
            enabled: true,
            confidence: None,
            evidence: None,
            review_required: false,
            evidence_kind: None,
        })
        .collect();
    let applied = service.apply(&media.session_id, clips).await?;
    let track = applied.mixed.ok_or("missing track")?;
    assert_eq!(track.comments.len(), 2);
    assert_eq!(track.comments[0].time, 22.0);
    let xml = tokio::fs::read_to_string(&track.file_path).await?;
    assert!(xml.contains("22.000,1,25,16777215"));
    let reopened = DanmakuService::new(
        playback.clone(),
        provider,
        dir.path().into(),
        dir.path().join("dm"),
    );
    assert_eq!(
        reopened
            .workspace(&media.session_id)
            .await?
            .mixed
            .ok_or("not persisted")?
            .comments
            .len(),
        2
    );
    playback.play_torrent("fixture", 4).await?;
    assert_eq!(
        reopened
            .workspace(&media.session_id)
            .await
            .err()
            .ok_or("accepted stale")?
            .code,
        "MEDIA_CHANGED"
    );
    playback.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn stopping_media_cancels_danmaku_work_and_releases_the_job(
) -> Result<(), Box<dyn std::error::Error>> {
    use bili_dm_lib::danmaku::service::DanmakuService;
    let (dir, _engine, _player, _library, playback) = setup().await?;
    let media = playback.play_torrent("fixture", 7).await?;
    playback.tick().await?;
    let analysis = playback.analysis_media(&media.session_id).await?;
    let provider = Arc::new(DanmakuProviderFixture {
        entered: Notify::new(),
    });
    let service = Arc::new(DanmakuService::new(
        playback.clone(),
        provider.clone(),
        dir.path().into(),
        dir.path().join("dm"),
    ));
    let work = {
        let service = service.clone();
        let session = media.session_id.clone();
        tokio::spawn(async move { service.resolve(&session, vec!["wait".into()]).await })
    };
    provider.entered.notified().await;
    assert!(service.status().running);
    assert_eq!(
        service
            .resolve(&media.session_id, vec!["a".into()])
            .await
            .err()
            .ok_or("allowed concurrency")?
            .code,
        "DANMAKU_BUSY"
    );
    playback.control(PlayerControl::Stop).await?;
    assert!(analysis.cancel.is_cancelled());
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), work)
            .await??
            .err()
            .ok_or("not cancelled")?
            .code,
        "CANCELLED"
    );
    assert!(!service.status().running);
    let reopened = playback.play_torrent("fixture", 7).await?;
    playback.tick().await?;
    assert_ne!(media.session_id, reopened.session_id);
    assert_eq!(
        service
            .resolve(&reopened.session_id, vec!["a".into()])
            .await?
            .sources
            .len(),
        1
    );
    playback.shutdown().await;
    Ok(())
}

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

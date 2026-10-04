//! Controlled loopback swarm + real libmpv; no public torrent dependency.
use axum::{routing::get, Router};
use bili_dm_lib::{
    core::playback_service::{PlaybackService, QueueOptions},
    database::{models::RepeatMode, sqlite::SqliteLibrary, LibraryRepository},
    player::{mpv::MpvBackend, PlayerControl},
    torrent::{rqbit::RqbitEngine, TorrentEngine},
};
use librqbit::{
    AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session, SessionOptions,
};
use std::{
    num::NonZeroU32,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::net::TcpListener;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[tokio::main]
async fn main() -> Result<()> {
    let fixture = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: streaming_smoke <fixture-directory> [hold-seeder]")?,
    );
    let root = tempfile::tempdir()?;
    let seed = Session::new_with_opts(
        fixture.clone(),
        SessionOptions {
            dht: None,
            disable_trackers: true,
            disable_local_service_discovery: true,
            listen: Some(ListenerOptions {
                listen_addr: ([127, 0, 0, 1], 0).into(),
                ..Default::default()
            }),
            ratelimits: librqbit::limits::LimitsConfig {
                upload_bps: NonZeroU32::new(1_500_000),
                download_bps: None,
            },
            ..Default::default()
        },
    )
    .await?;
    let address = seed.listen_addr().ok_or("no seed listener")?;
    let tracker = TcpListener::bind("127.0.0.1:0").await?;
    let tracker_url = format!("http://{}/announce", tracker.local_addr()?);
    let mut response = b"d8:intervali10e5:peers6:".to_vec();
    response.extend([127, 0, 0, 1]);
    response.extend(address.port().to_be_bytes());
    response.push(b'e');
    let router = Router::new().route(
        "/announce",
        get(move || {
            let body = response.clone();
            async move { body }
        }),
    );
    let tracker_task = tokio::spawn(async move {
        let _ = axum::serve(tracker, router).await;
    });
    let torrent = librqbit::create_torrent(
        &fixture,
        CreateTorrentOptions {
            name: Some("Bili DM controlled swarm"),
            piece_length: Some(256 * 1024),
            trackers: vec![tracker_url],
        },
        &librqbit::spawn_utils::BlockingSpawner::new(2),
    )
    .await?;
    let bytes = torrent.as_bytes()?;
    let seeded = seed
        .add_torrent(
            AddTorrent::from_bytes(bytes.clone()),
            Some(AddTorrentOptions {
                output_folder: Some(torrent.output_folder.to_string_lossy().into_owned()),
                overwrite: true,
                ..Default::default()
            }),
        )
        .await?
        .into_handle()
        .ok_or("seed not created")?;
    seeded.wait_until_initialized().await?;
    tokio::fs::write(root.path().join("fixture.torrent"), &bytes).await?;
    let engine = Arc::new(RqbitEngine::new(root.path().join("cache")));
    let player = Arc::new(MpvBackend::new(PathBuf::new()));
    let library = Arc::new(SqliteLibrary::open(&root.path().join("library.sqlite3"))?);
    let service = PlaybackService::new(engine.clone(), player, library.clone());
    let result = exercise(
        &service,
        &engine,
        library.as_ref(),
        torrent.as_magnet().to_string(),
    )
    .await;
    service.shutdown().await;
    if std::env::args().nth(2).as_deref() == Some("hold-seeder") && result.is_ok() {
        println!("GUI_MAGNET={}", torrent.as_magnet());
        println!(
            "GUI_TORRENT={}",
            root.path().join("fixture.torrent").display()
        );
        println!("SEED_READY; press Ctrl+C after native verification");
        tokio::signal::ctrl_c().await?;
    }
    seed.stop().await;
    tracker_task.abort();
    result
}
async fn wait(
    service: &PlaybackService,
    target: f64,
) -> Result<bili_dm_lib::core::playback_service::PlaybackState> {
    let started = Instant::now();
    loop {
        service.tick().await?;
        let state = service.snapshot().await?;
        if let Some(error) = &state.player.error {
            return Err(error.clone().into());
        }
        if state.player.loaded && !state.player.buffering && state.player.position >= target {
            return Ok(state);
        }
        if started.elapsed() > Duration::from_secs(100) {
            return Err(format!("playback timeout: {}", serde_json::to_string(&state)?).into());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
async fn exercise(
    service: &PlaybackService,
    engine: &RqbitEngine,
    library: &SqliteLibrary,
    magnet: String,
) -> Result<()> {
    let catalog = engine.resolve(&magnet).await?;
    library.save_catalog(&magnet, &catalog).await?;
    assert!(
        engine.download_stats().await.is_none(),
        "metadata started a body download"
    );
    let videos = bili_dm_lib::media::playlist::video_queue(&catalog.files);
    assert!(videos.len() >= 2, "fixture needs at least 2 episodes");
    service
        .queue_options(QueueOptions {
            auto_next: true,
            prefetch_next: true,
            repeat: RepeatMode::Off,
        })
        .await?;
    service.play_torrent(&catalog.id, videos[0].index).await?;
    let first = wait(service, 1.0).await?;
    let download = first.download.as_ref().ok_or("no download")?;
    assert!(
        download.downloaded < download.total,
        "not streamed before completion"
    );
    assert!(
        !download.selected_files.contains(&videos[1].index),
        "next selected too early"
    );
    println!(
        "PASS first frame at {:.1}% downloaded; queue={} files",
        100.0 * download.downloaded as f64 / download.total as f64,
        first.queue.len()
    );
    service
        .control(PlayerControl::Pause { paused: true })
        .await?;
    service.control(PlayerControl::Speed { speed: 1.5 }).await?;
    service
        .control(PlayerControl::Seek { seconds: 20.0 })
        .await?;
    service
        .control(PlayerControl::Pause { paused: false })
        .await?;
    let middle = wait(service, 21.0).await?;
    assert!(
        !middle
            .download
            .as_ref()
            .ok_or("download lost")?
            .buffered
            .is_empty(),
        "no verified piece map"
    );
    assert_eq!(middle.player.speed, 1.5);
    println!("PASS seek, speed and verified piece map");
    service.control(PlayerControl::Stop).await?;
    let stopped = service.snapshot().await?;
    assert!(stopped.media.is_none() && stopped.download.is_none());
    let resumed = service.play_torrent(&catalog.id, videos[0].index).await?;
    assert!(resumed.resumed_from >= 20.0, "resume was not persisted");
    let resumed_state = wait(service, 22.0).await?;
    assert!(resumed_state.player.position >= 20.0);
    println!(
        "PASS stop and per-file resume at {:.2}s",
        resumed.resumed_from
    );
    // Pause video, let the selected file finish, then verify one-ahead selection.
    service
        .control(PlayerControl::Pause { paused: true })
        .await?;
    let started = Instant::now();
    loop {
        service.tick().await?;
        let state = service.snapshot().await?;
        if let Some(stats) = &state.download {
            if stats.prefetch.is_some() {
                assert_eq!(stats.downloaded, stats.total);
                assert_eq!(
                    stats.prefetch.as_ref().map(|p| p.file_index),
                    Some(videos[1].index)
                );
                break;
            }
        }
        if started.elapsed() > Duration::from_secs(90) {
            return Err("prefetch did not start".into());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    println!("PASS one-ahead prefetch starts only after current file completes");
    let state = service.snapshot().await?;
    assert!(
        state
            .player
            .tracks
            .iter()
            .any(|t| t.kind == "sub" && t.external && t.selected),
        "companion subtitle not mounted and selected"
    );
    service
        .control(PlayerControl::Seek {
            seconds: state.player.duration - 0.5,
        })
        .await?;
    service
        .control(PlayerControl::Pause { paused: false })
        .await?;
    let started = Instant::now();
    loop {
        service.tick().await?;
        let state = service.snapshot().await?;
        if state.media.as_ref().and_then(|m| m.file_index) == Some(videos[1].index)
            && state.player.loaded
        {
            break;
        }
        if started.elapsed() > Duration::from_secs(40) {
            return Err("EOF did not advance".into());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    println!("PASS automatic companion subtitles and EOF next episode");
    service.control(PlayerControl::Stop).await?;
    assert!(engine.download_stats().await.is_none());
    Ok(())
}

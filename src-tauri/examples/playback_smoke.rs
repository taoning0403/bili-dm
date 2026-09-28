//! Real mpv + local/torrent smoke test. Uses a temporary download directory.
use bili_dm_lib::{
    core::playback_service::PlaybackService,
    player::{mpv::MpvBackend, PlayerControl},
    torrent::{rqbit::RqbitEngine, TorrentEngine},
};
use std::{path::PathBuf, sync::Arc, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .ok_or("usage: playback_smoke <local-video|magnet> [file-index]")?;
    let directory = tempfile::tempdir()?;
    let engine = Arc::new(RqbitEngine::new(directory.path().to_owned()));
    let player = Arc::new(MpvBackend::new(PathBuf::new()));
    let service = PlaybackService::new(engine.clone(), player);
    let result = exercise(&service, engine.as_ref(), input).await;
    service.shutdown().await;
    result
}

async fn exercise(
    service: &PlaybackService,
    engine: &RqbitEngine,
    input: String,
) -> Result<(), Box<dyn std::error::Error>> {
    if input.starts_with("magnet:") {
        let catalog = engine.resolve(&input).await?;
        let index = std::env::args()
            .nth(2)
            .map(|index| index.parse::<usize>())
            .transpose()?
            .or(catalog.suggested_file_index)
            .ok_or("no videos")?;
        println!(
            "catalog: {} files; selected index {index}",
            catalog.files.len()
        );
        service.play_torrent(&catalog.id, index).await?;
    } else {
        service.play_local(PathBuf::from(input)).await?;
    }
    let state = wait_for_playback(service, 2.0).await?;
    println!("playing: {}", serde_json::to_string(&state)?);
    service
        .control(PlayerControl::Pause { paused: true })
        .await?;
    service
        .control(PlayerControl::Volume { volume: 25.0 })
        .await?;
    let paused = service.snapshot().await?;
    if !paused.player.paused || (paused.player.volume - 25.0).abs() > 0.1 {
        return Err("pause/volume did not take effect".into());
    }
    let target = state.player.duration * 0.5;
    service
        .control(PlayerControl::Seek { seconds: target })
        .await?;
    service
        .control(PlayerControl::Pause { paused: false })
        .await?;
    let seeked = wait_for_playback(service, target + 1.0).await?;
    println!("seeked: {}", serde_json::to_string(&seeked)?);
    service.control(PlayerControl::Stop).await?;
    let stopped = service.snapshot().await?;
    if stopped.media.is_some() || stopped.download.is_some() {
        return Err("stop left an active media source".into());
    }
    println!("PASS: play, pause, volume, seek, resume, stop");
    Ok(())
}

async fn wait_for_playback(
    service: &PlaybackService,
    target: f64,
) -> Result<bili_dm_lib::core::playback_service::PlaybackState, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    loop {
        let state = service.snapshot().await?;
        if let Some(error) = &state.player.error {
            return Err(error.clone().into());
        }
        if state.player.loaded && !state.player.buffering && state.player.position >= target {
            return Ok(state);
        }
        if start.elapsed() > Duration::from_secs(110) {
            return Err(format!("playback timeout: {}", serde_json::to_string(&state)?).into());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

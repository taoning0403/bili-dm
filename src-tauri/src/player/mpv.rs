use super::{
    native::Client, Chapter, MediaTrack, PlayerBackend, PlayerControl, PlayerSnapshot,
    VideoViewport,
};
use crate::core::error::{AppError, AppResult};
use async_trait::async_trait;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

struct State {
    client: Option<Client>,
    viewport: VideoViewport,
}
pub struct MpvBackend {
    state: Arc<Mutex<State>>,
    library: PathBuf,
    wid: Option<i64>,
}
pub fn runtime_directory(resources: &Path) -> PathBuf {
    let bundled = resources.join("lib");
    if bundled.join(library_name()).is_file() {
        bundled
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib")
    }
}
pub(super) fn library_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "libmpv.dylib"
    } else if cfg!(windows) {
        "libmpv-2.dll"
    } else {
        "libmpv.so.2"
    }
}
impl MpvBackend {
    pub fn new(resources: PathBuf) -> Self {
        Self::with_target(resources, None)
    }
    pub fn embedded(resources: PathBuf, wid: i64) -> Self {
        Self::with_target(resources, Some(wid))
    }
    fn with_target(resources: PathBuf, wid: Option<i64>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                client: None,
                viewport: VideoViewport::default(),
            })),
            library: runtime_directory(&resources).join(library_name()),
            wid,
        }
    }
    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut State) -> AppResult<T> + Send + 'static,
    ) -> AppResult<T> {
        let state = self.state.clone();
        tokio::task::spawn_blocking(move || {
            let mut state = state
                .lock()
                .map_err(|_| AppError::new("PLAYER", "播放器状态锁不可用。"))?;
            work(&mut state)
        })
        .await
        .map_err(|e| AppError::new("PLAYER", e.to_string()))?
    }
}
fn margins(client: &mut Client, view: VideoViewport) -> AppResult<()> {
    for (name, value) in [
        ("left", view.left),
        ("right", view.right),
        ("top", view.top),
        ("bottom", view.bottom),
    ] {
        client.set(&format!("video-margin-ratio-{name}"), value)?;
    }
    Ok(())
}
#[async_trait]
impl PlayerBackend for MpvBackend {
    async fn load(&self, source: &str, start: f64) -> AppResult<()> {
        let (source, path, wid) = (source.to_owned(), self.library.clone(), self.wid);
        self.run(move |state| {
            if !start.is_finite() || start < 0.0 {
                return Err(AppError::new("PLAYER_ARGUMENT", "无效的续播位置。"));
            }
            if state.client.is_none() {
                state.client = Some(Client::open(&path, wid)?);
            }
            let client = state
                .client
                .as_mut()
                .ok_or_else(|| AppError::new("PLAYER", "播放器未就绪。"))?;
            client.drain_events();
            client.loaded = false;
            client.last_error = None;
            margins(client, state.viewport)?;
            client.set("pause", "no")?;
            client.set("aid", "auto")?;
            client.set("sid", "auto")?;
            client.command(&[
                "loadfile",
                &source,
                "replace",
                "-1",
                &format!("start={start}"),
            ])
        })
        .await
    }
    async fn add_subtitle(&self, source: &str, title: &str) -> AppResult<()> {
        let (source, title) = (source.to_owned(), title.to_owned());
        self.run(move |state| {
            let client = state
                .client
                .as_mut()
                .ok_or_else(|| AppError::new("NO_PLAYER", "请先打开视频。"))?;
            client.command(&["sub-add", &source, "auto", &title])
        })
        .await
    }
    async fn control(&self, control: PlayerControl) -> AppResult<()> {
        self.run(move |state| {
            let Some(client) = state.client.as_mut() else {
                return if matches!(control, PlayerControl::Stop) {
                    Ok(())
                } else {
                    Err(AppError::new("NO_PLAYER", "请先打开视频。"))
                };
            };
            match control {
                PlayerControl::Pause { paused } => {
                    client.set("pause", if paused { "yes" } else { "no" })
                }
                PlayerControl::Seek { seconds } if seconds.is_finite() && seconds >= 0.0 => {
                    client.command(&["seek", &seconds.to_string(), "absolute+exact"])
                }
                PlayerControl::Volume { volume }
                    if volume.is_finite() && (0.0..=100.0).contains(&volume) =>
                {
                    client.set("volume", volume)
                }
                PlayerControl::Mute { muted } => {
                    client.set("mute", if muted { "yes" } else { "no" })
                }
                PlayerControl::Speed { speed }
                    if speed.is_finite() && (0.25..=4.0).contains(&speed) =>
                {
                    client.set("speed", speed)
                }
                PlayerControl::AudioTrack { id } if id >= 0 => {
                    client.set("aid", if id == 0 { "no".into() } else { id.to_string() })
                }
                PlayerControl::SubtitleTrack { id } if id >= 0 => {
                    client.set("sid", if id == 0 { "no".into() } else { id.to_string() })
                }
                PlayerControl::SubtitleDelay { seconds }
                    if seconds.is_finite() && seconds.abs() <= 600.0 =>
                {
                    client.set("sub-delay", seconds)
                }
                PlayerControl::AudioDelay { seconds }
                    if seconds.is_finite() && seconds.abs() <= 600.0 =>
                {
                    client.set("audio-delay", seconds)
                }
                PlayerControl::FrameStep { backwards } => client.command(&[if backwards {
                    "frame-back-step"
                } else {
                    "frame-step"
                }]),
                PlayerControl::Stop => {
                    client.command(&["stop"])?;
                    client.loaded = false;
                    client.last_error = None;
                    Ok(())
                }
                _ => Err(AppError::new("INVALID_CONTROL", "播放器参数超出有效范围。")),
            }
        })
        .await
    }
    async fn viewport(&self, viewport: VideoViewport) -> AppResult<()> {
        if [viewport.left, viewport.right, viewport.top, viewport.bottom]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || viewport.left + viewport.right >= 1.0
            || viewport.top + viewport.bottom >= 1.0
        {
            return Err(AppError::new("INVALID_VIEWPORT", "无效的视频区域。"));
        }
        self.run(move |state| {
            state.viewport = viewport;
            if let Some(client) = &mut state.client {
                margins(client, viewport)?;
            }
            Ok(())
        })
        .await
    }
    async fn snapshot(&self) -> AppResult<PlayerSnapshot> {
        self.run(|state| {
            let Some(client) = &mut state.client else {
                return Ok(PlayerSnapshot::default());
            };
            client.drain_events();
            let number =
                |key| -> AppResult<f64> { Ok(client.property(key)?.as_f64().unwrap_or(0.0)) };
            let flag =
                |key| -> AppResult<bool> { Ok(client.property(key)?.as_bool().unwrap_or(false)) };
            let tracks = client
                .property("track-list")?
                .as_array()
                .into_iter()
                .flatten()
                .map(|t| MediaTrack {
                    id: t["id"].as_i64().unwrap_or_default(),
                    kind: t["type"].as_str().unwrap_or_default().into(),
                    title: t["title"].as_str().unwrap_or_default().into(),
                    language: t["lang"].as_str().unwrap_or_default().into(),
                    codec: t["codec"].as_str().unwrap_or_default().into(),
                    selected: t["selected"].as_bool().unwrap_or(false),
                    external: t["external"].as_bool().unwrap_or(false),
                })
                .collect();
            let chapters = client
                .property("chapter-list")?
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(index, c)| Chapter {
                    title: c["title"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("章节 {}", index + 1)),
                    time: c["time"].as_f64().unwrap_or_default(),
                })
                .collect();
            Ok(PlayerSnapshot {
                sampled_at_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64() * 1000.0)
                    .unwrap_or_default(),
                video_aspect: number("video-params/aspect")?,
                running: true,
                loaded: client.loaded,
                paused: flag("pause")?,
                buffering: flag("paused-for-cache")?,
                ended: flag("eof-reached")?,
                position: number("time-pos")?,
                duration: number("duration")?,
                volume: number("volume")?,
                muted: flag("mute")?,
                speed: number("speed")?,
                subtitle_delay: number("sub-delay")?,
                audio_delay: number("audio-delay")?,
                cache_seconds: number("demuxer-cache-duration")?,
                decoder: client
                    .property("hwdec-current")?
                    .as_str()
                    .unwrap_or("no")
                    .into(),
                tracks,
                chapters,
                error: client.last_error.clone(),
            })
        })
        .await
    }
    async fn shutdown(&self) {
        let _ = self
            .run(|state| {
                state.client.take();
                Ok(())
            })
            .await;
    }
}

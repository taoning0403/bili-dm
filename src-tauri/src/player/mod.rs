//! Player contracts, independent of torrent, database, and frontend.
pub mod audio_probe;
pub mod mpv;
mod native;
pub mod probe;
use crate::core::error::AppResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaTrack {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub language: String,
    pub codec: String,
    pub selected: bool,
    pub external: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Chapter {
    pub title: String,
    pub time: f64,
}
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub sampled_at_ms: f64,
    pub video_aspect: f64,
    pub running: bool,
    pub loaded: bool,
    pub paused: bool,
    pub buffering: bool,
    pub ended: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub subtitle_delay: f64,
    pub audio_delay: f64,
    pub cache_seconds: f64,
    pub decoder: String,
    pub tracks: Vec<MediaTrack>,
    pub chapters: Vec<Chapter>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlayerControl {
    Pause { paused: bool },
    Seek { seconds: f64 },
    Volume { volume: f64 },
    Mute { muted: bool },
    Speed { speed: f64 },
    AudioTrack { id: i64 },
    SubtitleTrack { id: i64 },
    SubtitleDelay { seconds: f64 },
    AudioDelay { seconds: f64 },
    FrameStep { backwards: bool },
    Stop,
}
#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub struct VideoViewport {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}
#[async_trait]
pub trait PlayerBackend: Send + Sync {
    async fn load(&self, source: &str, start: f64) -> AppResult<()>;
    async fn add_subtitle(&self, source: &str, title: &str) -> AppResult<()>;
    async fn control(&self, control: PlayerControl) -> AppResult<()>;
    async fn viewport(&self, viewport: VideoViewport) -> AppResult<()>;
    async fn snapshot(&self) -> AppResult<PlayerSnapshot>;
    async fn shutdown(&self);
}

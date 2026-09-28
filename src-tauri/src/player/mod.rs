//! Player contract. Adapters consume a source, never a torrent engine.
pub mod executable;
pub mod ipc;
pub mod mpv;

use crate::core::error::AppResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub running: bool,
    pub loaded: bool,
    pub paused: bool,
    pub buffering: bool,
    pub ended: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlayerControl {
    Pause { paused: bool },
    Seek { seconds: f64 },
    Volume { volume: f64 },
    Stop,
}

#[async_trait]
pub trait PlayerBackend: Send + Sync {
    async fn load(&self, source: &str) -> AppResult<()>;
    async fn control(&self, control: PlayerControl) -> AppResult<()>;
    async fn snapshot(&self) -> AppResult<PlayerSnapshot>;
    async fn shutdown(&self);
}

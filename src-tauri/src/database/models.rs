use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentRecord {
    pub torrent_id: String,
    pub magnet_uri: String,
    pub name: String,
    pub created_at: i64,
    pub status: String,
    pub file_count: u32,
}

pub struct MediaRecord {
    pub id: String,
    /// Canonical local path, or torrent://<info-hash>/<file-index>.
    pub path: String,
    pub filename: String,
    pub duration: Option<f64>,
    pub torrent_id: Option<String>,
    pub file_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackProgress {
    pub path: String,
    pub position: f64,
    pub duration: f64,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackPreference {
    pub disabled: bool,
    pub language: String,
    pub title: String,
    pub codec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlaybackPreferences {
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub auto_next: bool,
    pub prefetch_next: bool,
    pub repeat: RepeatMode,
    pub audio: Option<TrackPreference>,
    pub subtitle: Option<TrackPreference>,
}
impl Default for PlaybackPreferences {
    fn default() -> Self {
        Self {
            volume: 100.0,
            muted: false,
            speed: 1.0,
            auto_next: true,
            prefetch_next: true,
            repeat: RepeatMode::Off,
            audio: None,
            subtitle: None,
        }
    }
}

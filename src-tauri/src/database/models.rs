use serde::Serialize;

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

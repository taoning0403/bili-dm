use crate::media::files::MediaFile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentCatalog {
    pub id: String,
    pub name: String,
    pub files: Vec<MediaFile>,
    pub suggested_file_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DownloadStats {
    pub torrent_id: String,
    pub file_index: usize,
    pub downloaded: u64,
    pub total: u64,
    pub bytes_per_second: u64,
    pub peers: u32,
    pub state: String,
    pub error: Option<String>,
}

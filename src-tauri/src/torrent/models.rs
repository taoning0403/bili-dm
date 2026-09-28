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

use serde::{Deserialize, Serialize};

pub const MAX_COMMENTS: usize = 200_000;
pub const MAX_SOURCES: usize = 50;
pub const MAX_DURATION: f64 = 86_400.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub time: f64,
    pub mode: u32,
    pub color: u32,
    pub size: u32,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: String,
    pub bvid: String,
    pub cid: u64,
    pub page: u32,
    pub title: String,
    pub duration: f64,
    pub comment_count: usize,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub episode_id: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParsedSource {
    pub info: SourceInfo,
    pub comments: Vec<Comment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub source_id: String,
    pub source_start: f64,
    pub source_end: f64,
    pub target_start: f64,
    pub target_end: f64,
    pub enabled: bool,
    pub confidence: Option<f64>,
    pub evidence: Option<String>,
    #[serde(default)]
    pub review_required: bool,
    #[serde(default)]
    pub evidence_kind: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixedTrack {
    pub media_key: String,
    pub duration: f64,
    pub comments: Vec<Comment>,
    pub warnings: Vec<String>,
    pub file_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub media_key: String,
    pub duration: f64,
    pub sources: Vec<ParsedSource>,
    pub clips: Vec<Clip>,
    pub mixed: Option<MixedTrack>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub media_key: String,
    pub sources: Vec<SourceInfo>,
    pub clips: Vec<Clip>,
    pub mixed: Option<MixedTrack>,
    pub errors: Vec<String>,
}
impl Project {
    pub fn view(&self, errors: Vec<String>) -> Workspace {
        Workspace {
            media_key: self.media_key.clone(),
            sources: self.sources.iter().map(|s| s.info.clone()).collect(),
            clips: self.clips.clone(),
            mixed: self.mixed.clone(),
            errors,
        }
    }
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub running: bool,
    pub message: String,
    pub completed: usize,
    pub total: usize,
}

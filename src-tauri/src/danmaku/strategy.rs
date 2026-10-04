use crate::core::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceMode {
    #[default]
    Hybrid,
    Video,
    Audio,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchOptions {
    pub mode: EvidenceMode,
    pub allow_edits: bool,
    pub allow_redraw: bool,
    pub allow_audio_only: bool,
    pub min_duration_ratio: f64,
    pub min_coverage: f64,
    pub min_segment: f64,
    pub max_error: f64,
    pub sample_step: f64,
    pub max_candidates: usize,
    pub budget_seconds: u64,
    pub audio_track_id: Option<i64>,
}
impl Default for MatchOptions {
    fn default() -> Self {
        Self {
            mode: EvidenceMode::Hybrid,
            allow_edits: false,
            allow_redraw: true,
            allow_audio_only: false,
            min_duration_ratio: 1.0,
            min_coverage: 0.8,
            min_segment: 4.0,
            max_error: 0.5,
            sample_step: 2.0,
            max_candidates: 20,
            budget_seconds: 600,
            audio_track_id: None,
        }
    }
}
impl MatchOptions {
    pub fn validate(&self) -> AppResult<()> {
        let valid = [
            (self.min_duration_ratio, 0.0, 1.0),
            (self.min_coverage, 0.1, 1.0),
            (self.min_segment, 1.0, 60.0),
            (self.max_error, 0.1, 2.0),
            (self.sample_step, 0.5, 10.0),
        ]
        .iter()
        .all(|(n, min, max)| n.is_finite() && n >= min && n <= max);
        if !valid
            || !(1..=50).contains(&self.max_candidates)
            || !(30..=3600).contains(&self.budget_seconds)
            || self.audio_track_id.is_some_and(|id| id <= 0)
        {
            return Err(AppError::new(
                "MATCH_OPTIONS",
                "匹配策略参数无效，请检查时长、覆盖率、误差和分析预算。",
            ));
        }
        Ok(())
    }
    pub fn duration_allowed(&self, source: f64, target: f64) -> bool {
        source.is_finite()
            && target.is_finite()
            && source > 0.0
            && target > 0.0
            && source + 0.05 >= target * self.min_duration_ratio
    }
}

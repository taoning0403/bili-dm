use super::{
    alignment::{self, Evidence, Segment},
    audio::{self, AudioIndex},
    bilibili::{VideoStream, REFERER, USER_AGENT},
    matching,
    models::{Clip, SourceInfo},
    strategy::{EvidenceMode, MatchOptions},
};
use crate::{
    core::{
        error::{AppError, AppResult},
        playback_service::AnalysisMedia,
    },
    player::{
        audio_probe::AudioProbe,
        probe::{Frame, FrameProbe, ProbeInput, Progress},
    },
};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
pub struct MatchingEngine {
    frames: FrameProbe,
    audio: AudioProbe,
}
pub struct TargetIndex {
    input: ProbeInput,
    frames: Vec<Frame>,
    audio: Option<AudioIndex>,
    pub warnings: Vec<String>,
}
pub struct AlignedSource {
    pub clips: Vec<Clip>,
    pub coverage: f64,
    pub warnings: Vec<String>,
}
impl MatchingEngine {
    pub fn new(resources: PathBuf) -> Self {
        Self {
            frames: FrameProbe::new(resources.clone()),
            audio: AudioProbe::new(resources),
        }
    }
    pub async fn preview(
        &self,
        media: &AnalysisMedia,
        stream: VideoStream,
        source_time: f64,
        target_time: f64,
    ) -> AppResult<AlignmentPreview> {
        let source = self
            .frames
            .image(
                ProbeInput {
                    source: stream.url,
                    referer: Some(REFERER.into()),
                    user_agent: Some(USER_AGENT.into()),
                },
                source_time,
                media.cancel.clone(),
            )
            .await?;
        let target = self
            .frames
            .image(
                ProbeInput {
                    source: media.source.clone(),
                    referer: None,
                    user_agent: None,
                },
                target_time,
                media.cancel.clone(),
            )
            .await?;
        Ok(AlignmentPreview { source, target })
    }
    pub async fn prepare(
        &self,
        media: &AnalysisMedia,
        options: &MatchOptions,
        progress: Progress,
    ) -> AppResult<TargetIndex> {
        if media.duration > 10800.0 {
            return Err(AppError::new(
                "MATCH_LIMIT",
                "自动分析最长支持三小时；更长视频可手动指定区间。",
            ));
        }
        let input = ProbeInput {
            source: media.source.clone(),
            referer: None,
            user_agent: None,
        };
        let mut warnings = vec![];
        let frames = if options.mode != EvidenceMode::Audio {
            self.frames
                .sample(
                    input.clone(),
                    matching::sample_times(media.duration, options.sample_step),
                    media.cancel.clone(),
                    progress.clone(),
                )
                .await?
                .1
        } else {
            vec![]
        };
        let audio = if options.mode != EvidenceMode::Video {
            match self
                .audio_index(
                    input.clone(),
                    options.audio_track_id.or(media.audio_track_id),
                    media.duration,
                    &media.cancel,
                    progress,
                )
                .await
            {
                Ok(index) => Some(index),
                Err(e) if e.code == "CANCELLED" => return Err(e),
                Err(e) => {
                    warnings.push(e.message);
                    None
                }
            }
        } else {
            None
        };
        if options.mode == EvidenceMode::Audio && audio.is_none() {
            return Err(AppError::new("AUDIO_MISSING", warnings.join("；")));
        }
        Ok(TargetIndex {
            input,
            frames,
            audio,
            warnings,
        })
    }
    async fn audio_index(
        &self,
        input: ProbeInput,
        track: Option<i64>,
        duration: f64,
        cancel: &CancellationToken,
        progress: Progress,
    ) -> AppResult<AudioIndex> {
        let samples = self
            .audio
            .decode(input, track, duration, cancel.clone(), progress)
            .await?;
        let token = cancel.clone();
        tokio::task::spawn_blocking(move || audio::fingerprint_file(&samples, &token))
            .await
            .map_err(AppError::io)?
    }
    pub async fn match_source(
        &self,
        media: &AnalysisMedia,
        source: &SourceInfo,
        stream: VideoStream,
        target: &TargetIndex,
        options: &MatchOptions,
        progress: Progress,
    ) -> AppResult<AlignedSource> {
        if stream.duration > 10800.0 {
            return Err(AppError::new(
                "MATCH_LIMIT",
                "源视频超过三小时，请手动指定区间。",
            ));
        }
        let input = ProbeInput {
            source: stream.url,
            referer: Some(REFERER.into()),
            user_agent: Some(USER_AGENT.into()),
        };
        let mut warnings = vec![];
        let mut video = vec![];
        if options.mode != EvidenceMode::Audio {
            match self
                .frames
                .sample(
                    input.clone(),
                    matching::sample_times(stream.duration, options.sample_step),
                    media.cancel.clone(),
                    progress.clone(),
                )
                .await
            {
                Ok((duration, frames)) => {
                    if (duration - stream.duration).abs() > 2.0 {
                        return Err(AppError::new(
                            "MATCH_DURATION",
                            "解码时长与完整视频时长不符。",
                        ));
                    }
                    let anchors = alignment::visual_anchors(&frames, &target.frames);
                    let anchors = alignment::chain(
                        anchors,
                        options,
                        options.max_error.max(options.sample_step * 0.55),
                    );
                    let coarse = alignment::segments(
                        &anchors,
                        options.sample_step,
                        options.max_error.max(options.sample_step * 0.55),
                        options.min_segment,
                        stream.duration,
                        media.duration,
                    );
                    video = self
                        .refine_segments(
                            &input,
                            &target.input,
                            coarse,
                            options,
                            &media.cancel,
                            progress.clone(),
                            stream.duration,
                            media.duration,
                        )
                        .await?;
                }
                Err(e) if e.code == "CANCELLED" => return Err(e),
                Err(e) => warnings.push(format!("画面：{}", e.message)),
            }
        }
        let mut sound = vec![];
        if let (Some(target_audio), Some(url)) = (&target.audio, stream.audio_url) {
            let audio_input = ProbeInput {
                source: url,
                referer: Some(REFERER.into()),
                user_agent: Some(USER_AGENT.into()),
            };
            match self
                .audio_index(
                    audio_input,
                    None,
                    stream.duration,
                    &media.cancel,
                    progress.clone(),
                )
                .await
            {
                Ok(source_audio) => {
                    let anchors = audio::anchors(&source_audio, target_audio, &media.cancel)?;
                    let anchors = alignment::chain(anchors, options, options.max_error);
                    sound = alignment::segments(
                        &anchors,
                        0.5,
                        options.max_error,
                        options.min_segment,
                        stream.duration,
                        media.duration,
                    );
                }
                Err(e) if e.code == "CANCELLED" => return Err(e),
                Err(e) => warnings.push(format!("音频：{}", e.message)),
            }
        }
        if options.mode == EvidenceMode::Hybrid && !sound.is_empty() {
            // Audio supplies candidate positions, but each visual interval still
            // needs decoded start/middle/end evidence. This also disambiguates
            // repeated backgrounds without accepting unrelated music videos.
            let verified = self
                .refine_segments(
                    &input,
                    &target.input,
                    sound.clone(),
                    options,
                    &media.cancel,
                    progress.clone(),
                    stream.duration,
                    media.duration,
                )
                .await?;
            for mut segment in verified {
                segment.kind = Evidence::Video;
                video.push(segment);
            }
            video.sort_by(|a, b| a.source_start.total_cmp(&b.source_start));
        }
        if let Some(shift) = alignment::audio_delay(&video, &sound, options) {
            for segment in &mut sound {
                segment.offset += shift;
                segment.source_start = (segment.source_start + shift.abs())
                    .max(-segment.offset)
                    .max(0.0);
                segment.source_end =
                    (segment.source_end - shift.abs()).min(media.duration - segment.offset);
            }
            sound.retain(|s| s.source_end > s.source_start);
            warnings.push(format!(
                "依据连续画面校准固定音画延迟 {shift:+.2}s；音频区间边缘保留确认余量。"
            ));
        }
        let segments = alignment::fuse(&video, &sound, options);
        let coverage = alignment::coverage(&segments, media.duration);
        if segments.is_empty() {
            return Err(AppError::new(
                "MATCH_NOT_FOUND",
                format!("未找到可靠的有序匹配区间。{}", warnings.join("；")),
            ));
        }
        if segments.len() > 500 {
            return Err(AppError::new(
                "MATCH_LIMIT",
                "匹配区间超过 500 段，请增大最短区间或缩小范围。",
            ));
        }
        let insufficient = coverage + 0.001 < options.min_coverage;
        if insufficient {
            warnings.push(format!(
                "覆盖率 {:.1}% 低于设定 {:.1}%，结果需人工确认。",
                coverage * 100.0,
                options.min_coverage * 100.0
            ));
        }
        let clips = segments
            .into_iter()
            .map(|segment| {
                let kind = match segment.kind {
                    Evidence::Video => "video",
                    Evidence::Audio => "audio",
                    Evidence::Both => "both",
                };
                let label = match segment.kind {
                    Evidence::Video => "画面匹配",
                    Evidence::Audio => "音频匹配",
                    Evidence::Both => "音画一致",
                };
                let review = segment.review || insufficient;
                Clip {
                    id: uuid::Uuid::new_v4().to_string(),
                    source_id: source.id.clone(),
                    source_start: segment.source_start,
                    source_end: segment.source_end,
                    target_start: (segment.source_start + segment.offset).max(0.0),
                    target_end: (segment.source_end + segment.offset).min(media.duration),
                    enabled: !review,
                    confidence: Some(segment.score),
                    evidence: Some(format!(
                        "{label} · 偏移 {:+.2}s · 锚点偏移差 {:.2}s · 总覆盖 {:.1}%{}",
                        segment.offset,
                        segment.error,
                        coverage * 100.0,
                        if review {
                            " · 待确认：预览后勾选区间"
                        } else {
                            ""
                        }
                    )),
                    review_required: review,
                    evidence_kind: Some(kind.into()),
                }
            })
            .collect();
        Ok(AlignedSource {
            clips,
            coverage,
            warnings,
        })
    }
    #[allow(clippy::too_many_arguments)]
    async fn refine_segments(
        &self,
        source: &ProbeInput,
        target: &ProbeInput,
        coarse: Vec<Segment>,
        options: &MatchOptions,
        cancel: &CancellationToken,
        progress: Progress,
        source_duration: f64,
        target_duration: f64,
    ) -> AppResult<Vec<Segment>> {
        let mut queue = coarse;
        let mut result = vec![];
        let mut checked = 0;
        while let Some(segment) = queue.pop() {
            checked += 1;
            if checked > 160 {
                break;
            }
            let margin = 0.2;
            let middle = (segment.source_start + segment.source_end) / 2.0;
            let times = vec![
                (segment.source_start + margin).min(middle),
                middle,
                (segment.source_end - margin).max(middle),
            ];
            let (_, frames) = self
                .frames
                .sample(source.clone(), times, cancel.clone(), progress.clone())
                .await?;
            let mut target_times = vec![];
            for frame in &frames {
                let center = frame.time + segment.offset;
                let mut t = (center - options.sample_step - 0.5).max(0.1);
                while t < (center + options.sample_step + 0.5).min(target_duration - 0.1) {
                    target_times.push(t);
                    t += 0.25;
                }
            }
            target_times.sort_by(f64::total_cmp);
            target_times.dedup_by(|a, b| (*a - *b).abs() < 0.04);
            if target_times.is_empty() {
                continue;
            }
            let (_, fine) = self
                .frames
                .sample(
                    target.clone(),
                    target_times,
                    cancel.clone(),
                    progress.clone(),
                )
                .await?;
            let mut anchors = vec![];
            for frame in &frames {
                let best = fine
                    .iter()
                    .filter(|f| {
                        (f.time - frame.time - segment.offset).abs() <= options.sample_step + 0.6
                    })
                    .map(|f| (f, matching::distance(frame, f)))
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((f, d)) = best.filter(|(_, d)| *d < 0.13) {
                    anchors.push(alignment::Anchor {
                        source: frame.time,
                        target: f.time,
                        score: (1.0 - d / 0.2).clamp(0.0, 1.0),
                        kind: Evidence::Video,
                    });
                }
            }
            if anchors.len() == 3 {
                let low = anchors
                    .iter()
                    .map(|a| a.offset())
                    .fold(f64::INFINITY, f64::min);
                let high = anchors
                    .iter()
                    .map(|a| a.offset())
                    .fold(f64::NEG_INFINITY, f64::max);
                if high - low <= options.max_error {
                    let offset = anchors.iter().map(|a| a.offset()).sum::<f64>() / 3.0;
                    let mut s = segment;
                    s.offset = offset;
                    s.error = high - low;
                    s.score = anchors.iter().map(|a| a.score).sum::<f64>() / 3.0;
                    s.source_start = s.source_start.max(-offset).max(0.0);
                    s.source_end = s
                        .source_end
                        .min(source_duration)
                        .min(target_duration - offset);
                    result.push(s);
                    continue;
                }
            }
            // Recheck smaller intervals instead of stretching through a cut/redraw.
            if segment.source_end - segment.source_start >= options.min_segment * 2.0 {
                let mut left = segment.clone();
                left.source_end = middle;
                let mut right = segment;
                right.source_start = middle;
                queue.push(left);
                queue.push(right);
            }
        }
        result.sort_by(|a, b| a.source_start.total_cmp(&b.source_start));
        Ok(result)
    }
}
#[derive(serde::Serialize)]
pub struct AlignmentPreview {
    pub source: crate::player::probe::FrameImage,
    pub target: crate::player::probe::FrameImage,
}

use super::models::{Clip, MixedTrack, ParsedSource, MAX_COMMENTS, MAX_DURATION};
use crate::core::error::{AppError, AppResult};
use std::collections::{HashMap, HashSet};

pub fn mix(
    media_key: &str,
    duration: f64,
    sources: &[ParsedSource],
    clips: &[Clip],
) -> AppResult<MixedTrack> {
    if !duration.is_finite() || !(0.1..=MAX_DURATION).contains(&duration) || clips.len() > 500 {
        return Err(AppError::new("DANMAKU_RANGE", "视频时长或区间数量无效。"));
    }
    let sources: HashMap<_, _> = sources.iter().map(|s| (s.info.id.as_str(), s)).collect();
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut warnings = Vec::new();
    let mut coverage = Vec::new();
    for clip in clips.iter().filter(|c| c.enabled) {
        let source = sources.get(clip.source_id.as_str()).ok_or_else(|| {
            AppError::new("DANMAKU_SOURCE", "区间引用的弹幕源不存在，请重新解析。")
        })?;
        if [
            clip.source_start,
            clip.source_end,
            clip.target_start,
            clip.target_end,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || clip.source_start < 0.0
            || clip.target_start < 0.0
            || clip.source_end - clip.source_start < 0.05
            || clip.target_end - clip.target_start < 0.05
            || clip.source_end > source.info.duration + 0.05
            || clip.target_end > duration + 0.05
        {
            return Err(AppError::new(
                "DANMAKU_RANGE",
                format!(
                    "{}：区间必须位于源视频和当前视频时长以内，结束时间须大于开始时间。",
                    source.info.title
                ),
            ));
        }
        let scale = (clip.target_end - clip.target_start) / (clip.source_end - clip.source_start);
        if !(0.25..=4.0).contains(&scale) {
            return Err(AppError::new(
                "DANMAKU_RANGE",
                "源区间与目标区间的时长比例须在 0.25～4 倍以内。",
            ));
        }
        if (scale - 1.0).abs() > 0.005 {
            warnings.push(format!(
                "{}：按 {:.4} 倍时长映射弹幕，请检查同步。",
                source.info.title, scale
            ));
        }
        coverage.push((clip.target_start, clip.target_end));
        for comment in &source.comments {
            // Half-open intervals prevent duplicates at adjacent cuts.
            if comment.time < clip.source_start || comment.time >= clip.source_end {
                continue;
            }
            if !comment.time.is_finite()
                || !matches!(comment.mode, 1..=6)
                || comment.text.trim().is_empty()
            {
                continue;
            }
            let time = clip.target_start + (comment.time - clip.source_start) * scale;
            if time < 0.0 || time >= duration {
                continue;
            }
            // Only identical source IDs at the same mapped time are deduplicated.
            // Different users/sources posting the same text remain independent.
            let key = (
                source.info.id.clone(),
                comment.id.clone(),
                (time * 1000.0).round() as i64,
            );
            if seen.insert(key) {
                let mut mapped = comment.clone();
                mapped.time = time;
                mapped.id = format!("{}:{}:{:.3}", source.info.id, comment.id, time);
                result.push(mapped);
                if result.len() > MAX_COMMENTS {
                    return Err(AppError::new(
                        "DANMAKU_LIMIT",
                        "混合结果超过 20 万条，请缩小区间或减少弹幕源。",
                    ));
                }
            }
        }
    }
    if coverage.is_empty() {
        return Err(AppError::new("DANMAKU_RANGE", "请至少启用一个弹幕区间。"));
    }
    coverage.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut end: f64 = 0.0;
    let mut gap: f64 = 0.0;
    for (start, stop) in coverage {
        gap += (start - end).max(0.0);
        end = end.max(stop);
    }
    gap += (duration - end).max(0.0);
    if gap > 0.1 {
        warnings.push(format!("当前视频有 {:.1} 秒没有配置弹幕源区间。", gap));
    }
    result.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.id.cmp(&b.id)));
    if result.is_empty() {
        warnings.push("所选区间没有可显示的普通弹幕。".into());
    }
    Ok(MixedTrack {
        media_key: media_key.into(),
        duration,
        comments: result,
        warnings,
        file_path: String::new(),
    })
}

fn escape(value: &str) -> String {
    value
        .chars()
        .filter(|c| *c >= ' ' || matches!(c, '\n' | '\r' | '\t'))
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&apos;".into(),
            _ => c.to_string(),
        })
        .collect()
}
pub fn to_xml(track: &MixedTrack) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<i>\n");
    for (index, c) in track.comments.iter().enumerate() {
        xml.push_str(&format!(
            "<d p=\"{:.3},{},{},{},0,0,0,{}\">{}</d>\n",
            c.time,
            c.mode,
            c.size,
            c.color & 0xffffff,
            index + 1,
            escape(&c.text)
        ));
    }
    xml.push_str("</i>\n");
    xml
}

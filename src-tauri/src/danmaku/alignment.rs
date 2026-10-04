//! Ordered, piecewise alignment. Missing footage is represented by gaps.
use super::{matching, strategy::MatchOptions};
use crate::player::probe::Frame;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Evidence {
    Video,
    Audio,
    Both,
}
#[derive(Clone, Debug)]
pub struct Anchor {
    pub source: f64,
    pub target: f64,
    pub score: f64,
    pub kind: Evidence,
}
impl Anchor {
    pub fn offset(&self) -> f64 {
        self.target - self.source
    }
}
#[derive(Clone, Debug)]
pub struct Segment {
    pub source_start: f64,
    pub source_end: f64,
    pub offset: f64,
    pub score: f64,
    pub error: f64,
    pub kind: Evidence,
    pub review: bool,
}
fn hash(frame: &Frame) -> u64 {
    let mut bits = 0;
    for i in 0..64 {
        let a = frame.pixels[(i * 2) % frame.pixels.len()];
        let b = frame.pixels[(i * 2 + 1) % frame.pixels.len()];
        if a > b {
            bits |= 1 << i;
        }
    }
    bits
}
pub fn visual_anchors(source: &[Frame], target: &[Frame]) -> Vec<Anchor> {
    let target: Vec<_> = target
        .iter()
        .filter(|f| f.information >= 0.04 && !f.pixels.is_empty())
        .map(|f| (f, hash(f)))
        .collect();
    let mut result = vec![];
    for frame in source
        .iter()
        .filter(|f| f.information >= 0.04 && !f.pixels.is_empty())
    {
        let h = hash(frame);
        let mut choices: Vec<_> = target
            .iter()
            .filter(|(_, v)| (h ^ v).count_ones() <= 22)
            .map(|(f, _)| (*f, matching::distance(frame, f)))
            .filter(|(_, d)| *d < 0.14)
            .collect();
        choices.sort_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((best, score)) = choices.first() {
            if choices.iter().skip(1).any(|(other, d)| {
                (other.time - best.time).abs() > 5.0 && *d < *score * 1.15 + 0.0005
            }) {
                continue;
            }
            result.push(Anchor {
                source: frame.time,
                target: best.time,
                score: (1.0 - score / 0.2).clamp(0.0, 1.0),
                kind: Evidence::Video,
            });
        }
    }
    result
}
/// Find an ordered chain, allowing offset jumps only when edits are enabled.
pub fn chain(mut anchors: Vec<Anchor>, options: &MatchOptions, tolerance: f64) -> Vec<Anchor> {
    anchors.retain(|a| {
        a.source.is_finite() && a.target.is_finite() && a.source >= 0.0 && a.target >= 0.0
    });
    anchors.sort_by(|a, b| a.source.total_cmp(&b.source));
    let n = anchors.len();
    let mut scores = vec![0.0; n];
    let mut previous = vec![None; n];
    for i in 0..n {
        scores[i] = anchors[i].score.max(0.1);
        for j in i.saturating_sub(256)..i {
            let ds = anchors[i].source - anchors[j].source;
            let dt = anchors[i].target - anchors[j].target;
            if ds <= 0.05 || dt <= 0.05 {
                continue;
            }
            let jump = (dt - ds).abs();
            if jump > tolerance && !options.allow_edits {
                continue;
            }
            let cost = if jump > tolerance {
                1.5 + jump * 0.001
            } else {
                0.0
            };
            let score = scores[j] + anchors[i].score.max(0.1) - cost;
            if score > scores[i] {
                scores[i] = score;
                previous[i] = Some(j);
            }
        }
    }
    let Some((best, _)) = scores.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) else {
        return vec![];
    };
    let mut path = vec![];
    let mut cursor = Some(best);
    while let Some(i) = cursor {
        path.push(anchors[i].clone());
        cursor = previous[i];
    }
    path.reverse();
    path
}
pub fn segments(
    anchors: &[Anchor],
    step: f64,
    tolerance: f64,
    min_segment: f64,
    source_duration: f64,
    target_duration: f64,
) -> Vec<Segment> {
    let mut groups: Vec<Vec<&Anchor>> = vec![];
    for a in anchors {
        let same = groups.last().is_some_and(|g| {
            let first = g[0];
            let last = g[g.len() - 1];
            (a.offset() - first.offset()).abs() <= tolerance && a.source - last.source <= step * 2.6
        });
        if same {
            if let Some(group) = groups.last_mut() {
                group.push(a);
            }
        } else {
            groups.push(vec![a]);
        }
    }
    let mut result = vec![];
    for group in groups {
        if group.len() < 3 {
            continue;
        }
        let mut offsets: Vec<_> = group.iter().map(|a| a.offset()).collect();
        offsets.sort_by(f64::total_cmp);
        let offset = offsets[offsets.len() / 2];
        let error = offsets[offsets.len() - 1] - offsets[0];
        let start = (group[0].source - step / 2.0).max(0.0).max(-offset);
        let end = (group[group.len() - 1].source + step / 2.0)
            .min(source_duration)
            .min(target_duration - offset);
        if end - start < min_segment {
            continue;
        }
        result.push(Segment {
            source_start: start,
            source_end: end,
            offset,
            score: group.iter().map(|a| a.score).sum::<f64>() / group.len() as f64,
            error,
            kind: group[0].kind,
            review: false,
        });
    }
    // A deletion can occur inside a sampling interval. Never overlap source ranges.
    for i in 1..result.len() {
        if result[i - 1].source_end > result[i].source_start {
            let middle = (result[i - 1].source_end + result[i].source_start) / 2.0;
            result[i - 1].source_end = middle;
            result[i].source_start = middle;
        }
    }
    result
}
/// Audio-only support is reviewable unless strong visual evidence brackets it.
pub fn fuse(video: &[Segment], audio: &[Segment], options: &MatchOptions) -> Vec<Segment> {
    let mut boundaries: Vec<f64> = video
        .iter()
        .chain(audio)
        .flat_map(|s| [s.source_start, s.source_end])
        .collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    let mut result: Vec<Segment> = vec![];
    for range in boundaries.windows(2) {
        let middle = (range[0] + range[1]) / 2.0;
        let a = audio
            .iter()
            .find(|s| s.source_start <= middle && s.source_end > middle);
        let v = video
            .iter()
            .filter(|s| s.source_start <= middle && s.source_end > middle)
            .min_by(|x, y| {
                if let Some(a) = a {
                    (x.offset - a.offset)
                        .abs()
                        .total_cmp(&(y.offset - a.offset).abs())
                        .then(y.score.total_cmp(&x.score))
                } else {
                    y.score.total_cmp(&x.score)
                }
            });
        let selected = match (v, a) {
            (Some(v), Some(a)) if (v.offset - a.offset).abs() <= options.max_error => {
                let mut s = v.clone();
                s.offset = a.offset;
                s.kind = Evidence::Both;
                s.score = (v.score + a.score) / 2.0;
                s.error = v.error.max(a.error);
                Some(s)
            }
            (Some(v), Some(_)) => {
                let mut s = v.clone();
                s.review = true;
                Some(s)
            }
            (Some(v), None) => Some(v.clone()),
            (None, Some(a))
                if options.allow_redraw || options.mode == super::strategy::EvidenceMode::Audio =>
            {
                let before = video.iter().any(|v| {
                    v.source_end <= range[0] + 0.1
                        && (v.offset - a.offset).abs() <= options.max_error
                });
                let after = video.iter().any(|v| {
                    v.source_start >= range[1] - 0.1
                        && (v.offset - a.offset).abs() <= options.max_error
                });
                let mut s = a.clone();
                s.review = !(options.allow_audio_only || (before && after));
                Some(s)
            }
            _ => None,
        };
        if let Some(mut s) = selected {
            s.source_start = range[0];
            s.source_end = range[1];
            if let Some(last) = result.last_mut() {
                if last.kind == s.kind
                    && last.review == s.review
                    && (last.source_end - s.source_start).abs() < 0.01
                    && (last.offset - s.offset).abs() <= options.max_error.min(0.15)
                {
                    last.source_end = s.source_end;
                    continue;
                }
            }
            result.push(s);
        }
    }
    // Any overlap/reversal on the target timeline needs a human decision.
    for i in 1..result.len() {
        if result[i].source_start + result[i].offset
            < result[i - 1].source_end + result[i - 1].offset - 0.1
        {
            result[i].review = true;
            result[i - 1].review = true;
        }
    }
    result.retain(|s| s.source_end - s.source_start >= 0.5);
    result
}
/// Estimate only a small, consistent A/V delay backed by substantial visual
/// overlap. Local conflicts and large shifts remain reviewable, never averaged.
pub fn audio_delay(video: &[Segment], audio: &[Segment], options: &MatchOptions) -> Option<f64> {
    let mut evidence = vec![];
    let mut overlap = 0.0;
    for v in video {
        for a in audio {
            let length = v.source_end.min(a.source_end) - v.source_start.max(a.source_start);
            if length >= options.min_segment && v.score > 0.5 && a.score > 0.3 {
                evidence.push(v.offset - a.offset);
                overlap += length;
            }
        }
    }
    if overlap < 30.0 || evidence.is_empty() {
        return None;
    }
    evidence.sort_by(f64::total_cmp);
    let shift = evidence[evidence.len() / 2];
    if shift.abs() <= options.max_error
        || shift.abs() > 2.0
        || evidence[evidence.len() - 1] - evidence[0] > options.max_error / 2.0
    {
        return None;
    }
    Some(shift)
}
pub fn coverage(segments: &[Segment], duration: f64) -> f64 {
    let mut ranges: Vec<_> = segments
        .iter()
        .map(|s| {
            (
                (s.source_start + s.offset).max(0.0),
                (s.source_end + s.offset).min(duration),
            )
        })
        .collect();
    ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut end: f64 = 0.0;
    let mut total = 0.0;
    for (a, b) in ranges {
        total += (b - a.max(end)).max(0.0);
        end = end.max(b);
    }
    if duration > 0.0 {
        total / duration
    } else {
        0.0
    }
}

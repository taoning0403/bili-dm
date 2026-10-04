//! Conservative, deterministic visual matching. Duration alone is never a match.
use crate::player::probe::Frame;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Alignment {
    pub source_start: f64,
    pub source_end: f64,
    pub target_start: f64,
    pub target_end: f64,
    pub confidence: f64,
    pub head_score: f64,
    pub tail_score: f64,
    pub duration_error: f64,
}
pub fn distance(a: &Frame, b: &Frame) -> f64 {
    if a.information < 0.04
        || b.information < 0.04
        || a.pixels.len() != b.pixels.len()
        || a.pixels.is_empty()
    {
        return 1.0;
    }
    let n = a.pixels.len() as f64;
    let am = a.pixels.iter().map(|v| *v as f64).sum::<f64>() / n;
    let bm = b.pixels.iter().map(|v| *v as f64).sum::<f64>() / n;
    let raw = a
        .pixels
        .iter()
        .zip(&b.pixels)
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .sum::<f64>()
        / n;
    let normalized = a
        .pixels
        .iter()
        .zip(&b.pixels)
        .map(|(av, bv)| {
            (((*av as f64 - am) / a.information as f64)
                - ((*bv as f64 - bm) / b.information as f64))
                .abs()
        })
        .sum::<f64>()
        / n;
    raw * 0.65 + normalized * 0.07
}
pub fn sample_times(duration: f64, step: f64) -> Vec<f64> {
    let mut times = vec![];
    let mut time = 0.2;
    while time < duration - 0.1 {
        times.push(time);
        time += step;
    }
    if duration > 0.5 {
        times.push(duration - 0.2);
    }
    times
}
pub fn overlap(offset: f64, source_duration: f64, target_duration: f64) -> (f64, f64, f64) {
    let source_start = (-offset).max(0.0);
    let target_start = offset.max(0.0);
    (
        source_start,
        target_start,
        (source_duration - source_start).min(target_duration - target_start),
    )
}
fn nearest(frames: &[Frame], time: f64) -> Option<&Frame> {
    frames
        .iter()
        .min_by(|a, b| (a.time - time).abs().total_cmp(&(b.time - time).abs()))
}
pub fn candidates(
    source: &[Frame],
    target: &[Frame],
    source_duration: f64,
    target_duration: f64,
    step: f64,
) -> Vec<f64> {
    let mut votes: BTreeMap<i64, usize> = BTreeMap::new();
    for a in source.iter().filter(|a| a.information >= 0.04) {
        let mut neighbors: Vec<_> = target
            .iter()
            .map(|b| (distance(a, b), b.time - a.time))
            .filter(|(d, _)| *d < 0.14)
            .collect();
        neighbors.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, offset) in neighbors.iter().take(2) {
            *votes.entry((offset / step).round() as i64).or_default() += 1;
        }
    }
    let mut ranked = Vec::new();
    for (bucket, count) in votes {
        if count < 3 {
            continue;
        }
        let offset = bucket as f64 * step;
        let (ss, _, len) = overlap(offset, source_duration, target_duration);
        if len < source_duration.min(target_duration) * 0.6 || len < 4.0 {
            continue;
        }
        let scores: Vec<_> = [0.04, 0.25, 0.5, 0.75, 0.96]
            .iter()
            .filter_map(|fraction| {
                let a = nearest(source, ss + len * fraction)?;
                Some(distance(a, nearest(target, a.time + offset)?))
            })
            .collect();
        if scores.len() != 5 || scores.iter().filter(|s| **s < 0.18).count() < 3 {
            continue;
        }
        let score = scores.iter().sum::<f64>() / 5.0
            + 0.04 * (1.0 - len / source_duration.min(target_duration));
        ranked.push((score, offset));
    }
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
    ranked
        .into_iter()
        .take(3)
        .map(|(_, offset)| offset)
        .collect()
}

pub fn anchor_times(offset: f64, source_duration: f64, target_duration: f64) -> Vec<f64> {
    let (start, _, len) = overlap(offset, source_duration, target_duration);
    let margin = (len * 0.08).clamp(0.4, 3.0);
    vec![start + margin, start + len / 2.0, start + len - margin]
}
pub fn refine(
    source: &[Frame],
    target: &[Frame],
    offset: f64,
    span: f64,
    source_duration: f64,
    target_duration: f64,
) -> Option<Alignment> {
    if source.len() != 3 {
        return None;
    }
    let mut matches = Vec::new();
    for a in source {
        let (score, b) = target
            .iter()
            .filter(|b| (b.time - a.time - offset).abs() <= span + 0.3)
            .map(|b| (distance(a, b), b))
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        if score > 0.13 {
            return None;
        }
        matches.push((score, b.time - a.time));
    }
    let error = (matches[2].1 - matches[0].1).abs();
    if error > 0.85 || (matches[1].1 - matches[0].1).abs() > 0.85 {
        return None;
    }
    let offset = matches.iter().map(|(_, o)| o).sum::<f64>() / 3.0;
    let (ss, ts, len) = overlap(offset, source_duration, target_duration);
    if len < 4.0 {
        return None;
    }
    Some(Alignment {
        source_start: ss,
        source_end: ss + len,
        target_start: ts,
        target_end: ts + len,
        confidence: (1.0 - matches.iter().map(|(s, _)| s).sum::<f64>() / 3.0 / 0.2).clamp(0.0, 1.0),
        head_score: 1.0 - matches[0].0,
        tail_score: 1.0 - matches[2].0,
        duration_error: error,
    })
}

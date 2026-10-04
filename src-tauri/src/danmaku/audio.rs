//! Local spectral landmark fingerprints with time offsets. No recognition API.
use super::alignment::{Anchor, Evidence};
use crate::{
    core::error::{AppError, AppResult},
    player::audio_probe::{AudioSamples, DecodedAudio},
};
use rustfft::{num_complex::Complex, FftPlanner};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;
const FFT: usize = 1024;
const HOP: usize = 256;
use std::io::{BufReader, Read};
#[derive(Clone, Copy, Debug)]
pub struct Landmark {
    pub hash: u32,
    pub time: f64,
}
pub struct AudioIndex {
    pub marks: Vec<Landmark>,
    pub duration: f64,
}
pub fn fingerprint(audio: &AudioSamples, cancel: &CancellationToken) -> AppResult<AudioIndex> {
    fingerprint_blocks(
        audio
            .samples
            .as_chunks::<FFT>()
            .0
            .iter()
            .map(|chunk| Ok(chunk.to_vec())),
        audio.sample_rate,
        audio.samples.len() as f64 / audio.sample_rate as f64,
        cancel,
    )
}
pub fn fingerprint_file(audio: &DecodedAudio, cancel: &CancellationToken) -> AppResult<AudioIndex> {
    let mut reader = BufReader::new(std::fs::File::open(&audio.path).map_err(AppError::io)?);
    let blocks = std::iter::from_fn(move || {
        let mut bytes = [0u8; FFT * 2];
        match reader.read_exact(&mut bytes) {
            Ok(()) => Some(Ok(bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes(*b) as f32 / 32768.0)
                .collect::<Vec<_>>())),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => None,
            Err(e) => Some(Err(AppError::io(e))),
        }
    });
    fingerprint_blocks(blocks, audio.sample_rate, audio.duration, cancel)
}
fn fingerprint_blocks(
    blocks: impl Iterator<Item = AppResult<Vec<f32>>>,
    sample_rate: u32,
    duration: f64,
    cancel: &CancellationToken,
) -> AppResult<AudioIndex> {
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT);
    let window: Vec<f32> = (0..FFT)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FFT - 1) as f32).cos())
        .collect();
    let mut buffer = vec![Complex::new(0.0, 0.0); FFT];
    let mut scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];
    let mut peaks: Vec<Vec<usize>> = vec![];
    for (frame, block) in blocks.enumerate() {
        let samples = block?;
        if frame % 256 == 0 && cancel.is_cancelled() {
            return Err(AppError::new("CANCELLED", "已取消音频指纹分析。"));
        }
        let energy = samples.iter().map(|x| x * x).sum::<f32>() / FFT as f32;
        if energy < 0.000001 {
            peaks.push(vec![]);
            continue;
        }
        for i in 0..FFT {
            buffer[i] = Complex::new(samples[i] * window[i], 0.0);
        }
        fft.process_with_scratch(&mut buffer, &mut scratch);
        let spectrum: Vec<f32> = buffer[..FFT / 2].iter().map(|v| v.norm_sqr()).collect();
        let mut bins: Vec<usize> = (8..FFT / 2 - 2)
            .filter(|i| spectrum[*i] > spectrum[i - 1] && spectrum[*i] > spectrum[i + 1])
            .collect();
        bins.sort_by(|a, b| spectrum[*b].total_cmp(&spectrum[*a]));
        let mut chosen = vec![];
        for bin in bins {
            if chosen.iter().all(|v: &usize| v.abs_diff(bin) > 8) {
                chosen.push(bin);
            }
            if chosen.len() == 3 {
                break;
            }
        }
        peaks.push(chosen);
    }
    let step = (HOP * 4) as f64 / sample_rate as f64;
    let mut marks = vec![];
    for i in 0..peaks.len() {
        for delta in [3, 7] {
            if let Some(next) = peaks.get(i + delta) {
                for a in peaks[i].iter().take(2) {
                    for b in next.iter().take(2) {
                        // Coarse frequency bins tolerate codec/resampling changes.
                        let hash = ((*a as u32 / 3) << 18) | ((*b as u32 / 3) << 8) | delta as u32;
                        marks.push(Landmark {
                            hash,
                            time: i as f64 * step,
                        });
                    }
                }
            }
        }
    }
    Ok(AudioIndex { marks, duration })
}
pub fn anchors(
    source: &AudioIndex,
    target: &AudioIndex,
    cancel: &CancellationToken,
) -> AppResult<Vec<Anchor>> {
    let mut lookup: HashMap<u32, Vec<f64>> = HashMap::new();
    for mark in &target.marks {
        lookup.entry(mark.hash).or_default().push(mark.time);
    }
    // Common tones/music repetitions are weak evidence and must not dominate.
    lookup.retain(|_, times| times.len() <= 80);
    let mut result = vec![];
    let mut votes: HashMap<i64, (usize, f64, f64)> = HashMap::new();
    let mut window = -1;
    let flush = |votes: &mut HashMap<i64, (usize, f64, f64)>, result: &mut Vec<Anchor>| {
        let mut choices: Vec<_> = votes
            .values()
            .filter(|v| v.0 >= 4)
            .map(|(count, offset, time)| (*count, *offset / *count as f64, *time / *count as f64))
            .collect();
        choices.sort_by_key(|c| std::cmp::Reverse(c.0));
        if let Some(best) = choices.first() {
            let ambiguous = choices.iter().skip(1).any(|other| {
                (other.1 - best.1).abs() > 1.0 && other.0 as f64 > best.0 as f64 * 0.8
            });
            if !ambiguous {
                result.push(Anchor {
                    source: best.2,
                    target: best.2 + best.1,
                    score: (best.0 as f64 / 20.0).min(1.0),
                    kind: Evidence::Audio,
                });
            }
        }
        votes.clear();
    };
    for (i, mark) in source.marks.iter().enumerate() {
        if i % 2048 == 0 && cancel.is_cancelled() {
            return Err(AppError::new("CANCELLED", "已取消声音对齐。"));
        }
        let next = (mark.time / 0.5).floor() as i64;
        if next != window {
            flush(&mut votes, &mut result);
            window = next;
        }
        if let Some(times) = lookup.get(&mark.hash) {
            for time in times {
                let offset = time - mark.time;
                let entry = votes.entry((offset / 0.1).round() as i64).or_default();
                entry.0 += 1;
                entry.1 += offset;
                entry.2 += mark.time;
            }
        }
    }
    flush(&mut votes, &mut result);
    Ok(result)
}

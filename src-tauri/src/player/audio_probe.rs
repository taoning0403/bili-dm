//! Independent media audio decoder. No microphone/system-output capture.
use super::{
    mpv::{library_name, runtime_directory},
    native::Client,
    probe::{ProbeInput, Progress},
};
use crate::core::error::{AppError, AppResult};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
pub const SAMPLE_RATE: u32 = 11025;
pub struct AudioSamples {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}
pub struct DecodedAudio {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub duration: f64,
    _directory: tempfile::TempDir,
}
pub struct AudioProbe {
    library: PathBuf,
}
impl AudioProbe {
    pub fn new(resources: PathBuf) -> Self {
        Self {
            library: runtime_directory(&resources).join(library_name()),
        }
    }
    pub async fn decode(
        &self,
        input: ProbeInput,
        track: Option<i64>,
        duration: f64,
        cancel: CancellationToken,
        progress: Progress,
    ) -> AppResult<DecodedAudio> {
        let library = self.library.clone();
        tokio::task::spawn_blocking(move || {
            if !duration.is_finite() || !(0.1..=10800.0).contains(&duration) {
                return Err(AppError::new("AUDIO_LIMIT", "音频分析最长支持三小时。"));
            }
            let temp = tempfile::tempdir().map_err(AppError::io)?;
            let path = temp.path().join("analysis.pcm");
            let mut options = vec![
                ("vid", "no".into()),
                ("sid", "no".into()),
                ("ao", "pcm".into()),
                ("ao-pcm-file", path.to_string_lossy().into_owned()),
                ("ao-pcm-waveheader", "no".into()),
                ("audio-format", "s16".into()),
                ("audio-samplerate", SAMPLE_RATE.to_string()),
                ("audio-channels", "mono".into()),
                ("audio-display", "no".into()),
                ("untimed", "yes".into()),
                ("keep-open", "no".into()),
                ("pause", "no".into()),
                ("audio-delay", "0".into()),
                ("speed", "1".into()),
                ("volume", "100".into()),
                ("mute", "no".into()),
                ("end", (duration + 1.0).to_string()),
                ("network-timeout", "10".into()),
                ("demuxer-max-bytes", "8MiB".into()),
                ("cache-secs", "2".into()),
            ];
            if let Some(id) = track {
                options.push(("aid", id.to_string()));
            }
            if let Some(referer) = input.referer {
                options.push(("referrer", referer));
            }
            if let Some(agent) = input.user_agent {
                options.push(("user-agent", agent));
            }
            let mut client = Client::open_with_options(&library, None, &options)?;
            client.command(&["loadfile", &input.source, "replace"])?;
            let started = Instant::now();
            let mut last_progress = Instant::now();
            let mut last_position = 0.0;
            loop {
                if cancel.is_cancelled() {
                    return Err(AppError::new("CANCELLED", "已取消音频分析。"));
                }
                client.drain_events();
                if client.last_error.is_some() {
                    return Err(AppError::new(
                        "AUDIO_DECODE",
                        "音频解码失败，可能无法访问该音轨。",
                    ));
                }
                if client.finished {
                    break;
                }
                let position = client.property("time-pos")?.as_f64().unwrap_or(0.0);
                if position > last_position + 0.1 {
                    last_position = position;
                    last_progress = Instant::now();
                    progress(position as usize, duration.ceil() as usize);
                }
                if last_progress.elapsed() > Duration::from_secs(30)
                    || started.elapsed() > Duration::from_secs(900)
                {
                    return Err(AppError::new(
                        "AUDIO_TIMEOUT",
                        "音频读取超时，请检查分片或缩小分析范围。",
                    ));
                }
                if std::fs::metadata(&path)
                    .is_ok_and(|m| m.len() > ((duration + 5.0) * SAMPLE_RATE as f64 * 2.0) as u64)
                {
                    return Err(AppError::new("AUDIO_LIMIT", "音频输出超过时长限制。"));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            drop(client); // Flush PCM and close the decoder before reading/deleting it.
            let length = std::fs::metadata(&path)
                .map_err(|_| {
                    AppError::new(
                        "AUDIO_MISSING",
                        "没有可解码的音轨，请选择其他音轨或使用画面匹配。",
                    )
                })?
                .len();
            if length < SAMPLE_RATE as u64
                || length > ((duration + 5.0) * SAMPLE_RATE as f64 * 2.0) as u64
            {
                return Err(AppError::new("AUDIO_DATA", "音频长度无效。"));
            }
            Ok(DecodedAudio {
                path,
                sample_rate: SAMPLE_RATE,
                duration: length as f64 / (SAMPLE_RATE as f64 * 2.0),
                _directory: temp,
            })
        })
        .await
        .map_err(AppError::io)?
    }
}

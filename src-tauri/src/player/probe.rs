//! An isolated decoder for visual analysis; never seeks the playback instance.
use super::{
    mpv::{library_name, runtime_directory},
    native::Client,
};
use crate::core::error::{AppError, AppResult};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ProbeInput {
    pub source: String,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Frame {
    pub time: f64,
    pub pixels: Vec<f32>,
    pub information: f32,
}
pub type Progress = Arc<dyn Fn(usize, usize) + Send + Sync>;

pub struct FrameProbe {
    library: PathBuf,
}
impl FrameProbe {
    pub fn new(resources: PathBuf) -> Self {
        Self {
            library: runtime_directory(&resources).join(library_name()),
        }
    }
    pub async fn sample(
        &self,
        input: ProbeInput,
        times: Vec<f64>,
        cancel: CancellationToken,
        progress: Progress,
    ) -> AppResult<(f64, Vec<Frame>)> {
        let library = self.library.clone();
        // Await this worker even on cancellation so the decoder and its readers
        // are destroyed before another job can begin.
        tokio::task::spawn_blocking(move || {
            if times.is_empty()
                || times.len() > 2000
                || times.iter().any(|t| !t.is_finite() || *t < 0.0)
            {
                return Err(AppError::new("FRAME_INPUT", "取帧时间无效。"));
            }
            let mut client = Client::open(&library, None)?;
            for (key, value) in [
                ("pause", "yes"),
                ("aid", "no"),
                ("sid", "no"),
                ("sub-auto", "no"),
                ("hwdec", "no"),
                ("vf", "scale=160:90"),
                ("network-timeout", "10"),
                ("demuxer-max-bytes", "8MiB"),
                ("cache-secs", "2"),
            ] {
                client.set(key, value)?;
            }
            if let Some(referer) = input.referer {
                client.set("referrer", referer)?;
            }
            if let Some(agent) = input.user_agent {
                client.set("user-agent", agent)?;
            }
            client.command(&[
                "loadfile",
                &input.source,
                "replace",
                "-1",
                &format!("start={}", times[0]),
            ])?;
            wait_frame(&mut client, 0, &cancel)?;
            let duration = client.property("duration")?.as_f64().unwrap_or(0.0);
            if !duration.is_finite() || duration <= 0.0 {
                return Err(AppError::new("FRAME_DURATION", "无法读取视频时长。"));
            }
            let mut frames = Vec::with_capacity(times.len());
            for (index, time) in times.iter().enumerate() {
                if cancel.is_cancelled() {
                    return Err(AppError::new("CANCELLED", "已取消画面分析。"));
                }
                let time = time.min((duration - 0.1).max(0.0));
                if index > 0 {
                    let revision = client.frame_revision;
                    client.command(&["seek", &time.to_string(), "absolute+exact"])?;
                    wait_frame(&mut client, revision, &cancel)?;
                }
                let actual = client.property("time-pos")?.as_f64().unwrap_or(time);
                if (actual - time).abs() > 1.0 {
                    return Err(AppError::new(
                        "FRAME_SEEK",
                        "视频未能准确定位，可能缺少数据分片，请稍后重试或手动指定区间。",
                    ));
                }
                let (w, h, rgb) = client.screenshot_rgb()?;
                frames.push(fingerprint(actual, w, h, &rgb));
                progress(index + 1, times.len());
            }
            Ok((duration, frames))
        })
        .await
        .map_err(|e| AppError::new("FRAME_WORKER", e.to_string()))?
    }
}
fn wait_frame(client: &mut Client, previous: u64, cancel: &CancellationToken) -> AppResult<()> {
    let started = Instant::now();
    loop {
        if cancel.is_cancelled() {
            return Err(AppError::new("CANCELLED", "已取消画面分析。"));
        }
        client.drain_events();
        if let Some(error) = &client.last_error {
            return Err(AppError::new(
                "FRAME_DECODE",
                format!("视频取帧失败：{error}"),
            ));
        }
        if client.loaded
            && client.frame_revision != previous
            && client.property("seeking")?.as_bool() != Some(true)
        {
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(20) {
            return Err(AppError::new(
                "FRAME_TIMEOUT",
                "视频取帧超时，可能尚未下载对应分片或 B 站视频流不可访问。可重试或手动指定区间。",
            ));
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

pub fn fingerprint(time: f64, width: usize, height: usize, rgb: &[u8]) -> Frame {
    // Average cells, avoiding the outer 8% (letterbox edges and corner logos).
    let mut pixels = Vec::with_capacity(16 * 9);
    for row in 0..9 {
        for col in 0..16 {
            let x0 = width * (8 * 16 + col * 84) / 1600;
            let x1 = (width * (8 * 16 + (col + 1) * 84) / 1600)
                .max(x0 + 1)
                .min(width);
            let y0 = height * (8 * 9 + row * 84) / 900;
            let y1 = (height * (8 * 9 + (row + 1) * 84) / 900)
                .max(y0 + 1)
                .min(height);
            let mut value = 0.0;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * width + x) * 3;
                    value += (rgb[i] as f32 * 0.299
                        + rgb[i + 1] as f32 * 0.587
                        + rgb[i + 2] as f32 * 0.114)
                        / 255.0;
                }
            }
            pixels.push(value / ((x1 - x0) * (y1 - y0)) as f32);
        }
    }
    let mean = pixels.iter().sum::<f32>() / pixels.len() as f32;
    let information =
        (pixels.iter().map(|p| (p - mean).powi(2)).sum::<f32>() / pixels.len() as f32).sqrt();
    Frame {
        time,
        pixels,
        information,
    }
}

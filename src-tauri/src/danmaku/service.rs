use super::{
    bilibili::{cancelled, DanmakuProvider, REFERER, USER_AGENT},
    matching, mixer,
    models::*,
};
use crate::{
    core::{
        error::{AppError, AppResult},
        playback_service::{AnalysisMedia, PlaybackService},
    },
    player::probe::{FrameProbe, ProbeInput, Progress},
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

pub struct DanmakuService {
    playback: Arc<PlaybackService>,
    provider: Arc<dyn DanmakuProvider>,
    probe: FrameProbe,
    directory: PathBuf,
    serial: Arc<Semaphore>,
    status: Arc<Mutex<JobStatus>>,
    cancellation: Mutex<Option<CancellationToken>>,
}
struct MatchIndex {
    input: ProbeInput,
    duration: f64,
    frames: Vec<crate::player::probe::Frame>,
    step: f64,
}
struct Job {
    _permit: OwnedSemaphorePermit,
    status: Arc<Mutex<JobStatus>>,
    cancel: CancellationToken,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Ok(mut status) = self.status.lock() {
            status.running = false;
        }
    }
}
impl DanmakuService {
    pub fn new(
        playback: Arc<PlaybackService>,
        provider: Arc<dyn DanmakuProvider>,
        resources: PathBuf,
        directory: PathBuf,
    ) -> Self {
        Self {
            playback,
            provider,
            probe: FrameProbe::new(resources),
            directory,
            serial: Arc::new(Semaphore::new(1)),
            status: Arc::new(Mutex::new(JobStatus::default())),
            cancellation: Mutex::new(None),
        }
    }
    pub fn status(&self) -> JobStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn cancel(&self) {
        if let Ok(token) = self.cancellation.lock() {
            if let Some(token) = &*token {
                token.cancel();
            }
        }
    }
    fn report(&self, message: impl Into<String>, completed: usize, total: usize) {
        if let Ok(mut status) = self.status.lock() {
            *status = JobStatus {
                running: true,
                message: message.into(),
                completed,
                total,
            };
        }
    }
    async fn begin(&self, session: &str) -> AppResult<(AnalysisMedia, Job)> {
        let permit = self
            .serial
            .clone()
            .try_acquire_owned()
            .map_err(|_| AppError::new("DANMAKU_BUSY", "弹幕任务正在运行，请等待或取消。"))?;
        let media = self.playback.analysis_media(session).await?;
        if let Ok(mut cancel) = self.cancellation.lock() {
            *cancel = Some(media.cancel.clone());
        }
        self.report("准备弹幕任务…", 0, 0);
        let job = Job {
            _permit: permit,
            status: self.status.clone(),
            cancel: media.cancel.clone(),
        };
        Ok((media, job))
    }
    fn path(&self, key: &str, extension: &str) -> PathBuf {
        self.directory
            .join(format!("{:x}.{extension}", md5::compute(key)))
    }
    async fn read(&self, media: &AnalysisMedia) -> AppResult<Project> {
        let path = self.path(&media.key, "json");
        let metadata = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Project {
                    schema_version: 1,
                    media_key: media.key.clone(),
                    duration: media.duration,
                    sources: vec![],
                    clips: vec![],
                    mixed: None,
                })
            }
            Err(e) => return Err(AppError::io(e)),
        };
        if metadata.len() > 128 * 1024 * 1024 {
            return Err(AppError::new(
                "DANMAKU_STORAGE",
                "本地弹幕项目超过大小限制。",
            ));
        }
        let bytes = tokio::fs::read(path).await.map_err(AppError::io)?;
        let mut project: Project = serde_json::from_slice(&bytes).map_err(|_| {
            AppError::new("DANMAKU_STORAGE", "本地弹幕项目无法读取，请检查项目文件。")
        })?;
        if project.schema_version != 1
            || project.media_key != media.key
            || project.sources.len() > MAX_SOURCES
            || project
                .sources
                .iter()
                .map(|s| s.comments.len())
                .sum::<usize>()
                > MAX_COMMENTS
            || project.clips.len() > 500
        {
            return Err(AppError::new(
                "DANMAKU_STORAGE",
                "本地弹幕项目版本或内容无效。",
            ));
        }
        if (project.duration - media.duration).abs() > 0.5 {
            project.mixed = None;
        }
        project.duration = media.duration;
        if let Some(track) = &mut project.mixed {
            track.file_path = self.path(&media.key, "xml").to_string_lossy().into_owned();
        }
        Ok(project)
    }
    async fn save(&self, project: &Project, cancel: &CancellationToken) -> AppResult<()> {
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        tokio::fs::create_dir_all(&self.directory)
            .await
            .map_err(AppError::io)?;
        let bytes = serde_json::to_vec(project).map_err(AppError::io)?;
        atomic_write(&self.path(&project.media_key, "json"), &bytes).await
    }
    pub async fn workspace(&self, session: &str) -> AppResult<Workspace> {
        let media = self.playback.analysis_media(session).await?;
        Ok(self.read(&media).await?.view(vec![]))
    }
    pub async fn resolve(&self, session: &str, inputs: Vec<String>) -> AppResult<Workspace> {
        if inputs.is_empty() || inputs.len() > 20 || inputs.iter().any(|s| s.len() > 2048) {
            return Err(AppError::new(
                "BILIBILI_INPUT",
                "每批请输入 1～20 个 BV 号或链接，每行一个。",
            ));
        }
        let (media, _job) = self.begin(session).await?;
        let mut project = self.read(&media).await?;
        let mut errors = Vec::new();
        let mut requested = HashSet::new();
        for (index, input) in inputs.iter().enumerate() {
            self.report(
                format!("解析第 {} / {} 个链接", index + 1, inputs.len()),
                index,
                inputs.len(),
            );
            let videos = match self.provider.videos(input, &media.cancel).await {
                Ok(v) => v,
                Err(e) if e.code == "CANCELLED" => return Err(e),
                Err(e) => {
                    errors.push(format!("第 {} 个输入：{}", index + 1, e.message));
                    continue;
                }
            };
            for video in videos {
                if !requested.insert(video.id.clone()) {
                    continue;
                }
                if project.sources.len() >= MAX_SOURCES
                    && !project.sources.iter().any(|s| s.info.id == video.id)
                {
                    errors.push("弹幕源最多 50 个分 P，请使用链接的 ?p= 指定分 P。".into());
                    break;
                }
                self.report(format!("读取弹幕：{}", video.title), index, inputs.len());
                match self.provider.comments(video.clone(), &media.cancel).await {
                    Ok(parsed) => {
                        let count: usize = project
                            .sources
                            .iter()
                            .filter(|s| s.info.id != video.id)
                            .map(|s| s.comments.len())
                            .sum();
                        if count + parsed.comments.len() > MAX_COMMENTS {
                            errors.push(format!("{}：项目弹幕总量超过 20 万条。", video.title));
                            continue;
                        }
                        if let Some(existing) =
                            project.sources.iter_mut().find(|s| s.info.id == video.id)
                        {
                            *existing = parsed;
                        } else {
                            project.sources.push(parsed);
                        }
                    }
                    Err(e) if e.code == "CANCELLED" => return Err(e),
                    Err(e) => errors.push(format!("{}：{}", video.title, e.message)),
                }
            }
        }
        // An existing applied track remains stable until the user remixes.
        self.save(&project, &media.cancel).await?;
        Ok(project.view(errors))
    }
    fn progress(&self, label: String) -> Progress {
        let status = self.status.clone();
        Arc::new(move |completed, total| {
            if let Ok(mut status) = status.lock() {
                *status = JobStatus {
                    running: true,
                    message: label.clone(),
                    completed,
                    total,
                };
            }
        })
    }
    pub async fn auto_match(
        &self,
        session: &str,
        source_ids: Vec<String>,
    ) -> AppResult<MatchResult> {
        let (media, _job) = self.begin(session).await?;
        let project = self.read(&media).await?;
        let selected: HashSet<_> = source_ids.into_iter().collect();
        let sources: Vec<_> = project
            .sources
            .iter()
            .filter(|s| selected.contains(&s.info.id))
            .collect();
        if sources.is_empty() {
            return Err(AppError::new("DANMAKU_SOURCE", "请至少选择一个弹幕源。"));
        }
        if media.duration > 7200.0 {
            return Err(AppError::new(
                "MATCH_LIMIT",
                "自动匹配支持两小时以内的视频；长视频请手动指定区间。",
            ));
        }
        // Unrelated long sources must not lower the target's sampling density.
        let step = (media.duration / 240.0).clamp(1.0, 30.0);
        let target_input = ProbeInput {
            source: media.source.clone(),
            referer: None,
            user_agent: None,
        };
        self.report("读取当前视频画面索引…", 0, 0);
        let (target_duration, target_frames) = self
            .probe
            .sample(
                target_input.clone(),
                matching::sample_times(media.duration, step),
                media.cancel.clone(),
                self.progress("读取当前视频画面索引".into()),
            )
            .await?;
        if (target_duration - media.duration).abs() > 1.0 {
            return Err(AppError::new(
                "MATCH_DURATION",
                "当前视频解码时长发生变化，请重新加载后匹配。",
            ));
        }
        let target_index = MatchIndex {
            input: target_input,
            duration: target_duration,
            frames: target_frames,
            step,
        };
        let mut result = MatchResult {
            clips: vec![],
            errors: vec![],
        };
        for (index, source) in sources.iter().enumerate() {
            self.report(
                format!(
                    "匹配 {} / {}：{}",
                    index + 1,
                    sources.len(),
                    source.info.title
                ),
                0,
                0,
            );
            let matched = self.match_source(&media, &source.info, &target_index).await;
            match matched {
                Ok(clip) => result.clips.push(clip),
                Err(e) if e.code == "CANCELLED" => return Err(e),
                Err(e) => result
                    .errors
                    .push(format!("{}：{}", source.info.title, e.message)),
            }
        }
        if media.cancel.is_cancelled() {
            return Err(cancelled());
        }
        Ok(result)
    }
    async fn match_source(
        &self,
        media: &AnalysisMedia,
        source: &SourceInfo,
        index: &MatchIndex,
    ) -> AppResult<Clip> {
        if source.duration > 7200.0 {
            return Err(AppError::new(
                "MATCH_LIMIT",
                "源视频超过两小时，请手动指定区间。",
            ));
        }
        let stream = self.provider.stream(source, &media.cancel).await?;
        let source_step = index.step.max(stream.duration / 600.0);
        let input = ProbeInput {
            source: stream.url,
            referer: Some(REFERER.into()),
            user_agent: Some(USER_AGENT.into()),
        };
        let (duration, frames) = self
            .probe
            .sample(
                input.clone(),
                matching::sample_times(stream.duration, source_step),
                media.cancel.clone(),
                self.progress(format!("读取源画面：{}", source.title)),
            )
            .await?;
        if (duration - stream.duration).abs() > 1.0 {
            return Err(AppError::new(
                "MATCH_DURATION",
                "源视频实际时长与接口不符，不能自动匹配。",
            ));
        }
        let offsets =
            matching::candidates(&frames, &index.frames, duration, index.duration, index.step);
        let mut accepted = Vec::new();
        for offset in offsets {
            let times = matching::anchor_times(offset, duration, index.duration);
            let (_, anchors) = self
                .probe
                .sample(
                    input.clone(),
                    times,
                    media.cancel.clone(),
                    self.progress("精确校验源首尾与中段画面".into()),
                )
                .await?;
            let span = source_step + 0.5;
            let mut times = Vec::new();
            for anchor in &anchors {
                let center = anchor.time + offset;
                let mut t = (center - span).max(0.1);
                while t <= (center + span).min(index.duration - 0.1) {
                    times.push(t);
                    t += 0.25;
                }
            }
            times.sort_by(f64::total_cmp);
            times.dedup_by(|a, b| (*a - *b).abs() < 0.05);
            let (_, fine) = self
                .probe
                .sample(
                    index.input.clone(),
                    times,
                    media.cancel.clone(),
                    self.progress("精确校验当前视频首尾与中段".into()),
                )
                .await?;
            if let Some(alignment) =
                matching::refine(&anchors, &fine, offset, span, duration, index.duration)
            {
                accepted.push(alignment);
            }
        }
        accepted.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        let best = accepted.first().ok_or_else(|| {
            AppError::new(
                "MATCH_NOT_FOUND",
                "没有找到首尾、中段画面及区间时长均一致的匹配，请手动添加区间。",
            )
        })?;
        if accepted.get(1).is_some_and(|other| {
            (other.confidence - best.confidence).abs() < 0.05
                && (other.target_start - other.source_start - best.target_start + best.source_start)
                    .abs()
                    > 1.0
        }) {
            return Err(AppError::new(
                "MATCH_AMBIGUOUS",
                "发现重复画面或多个相近候选，请手动指定区间。",
            ));
        }
        Ok(Clip { id:uuid::Uuid::new_v4().to_string(),source_id:source.id.clone(),source_start:best.source_start,source_end:best.source_end.min(source.duration),target_start:best.target_start,target_end:best.target_end.min(media.duration),enabled:true,confidence:Some(best.confidence),evidence:Some(format!("首帧 {:.0}% · 尾帧 {:.0}% · 中段通过 · 首尾时差 {:.2}s；采样精度约 0.25s，请预览复核。",best.head_score*100.0,best.tail_score*100.0,best.duration_error)) })
    }
    pub async fn apply(&self, session: &str, clips: Vec<Clip>) -> AppResult<Workspace> {
        let (media, _job) = self.begin(session).await?;
        let mut project = self.read(&media).await?;
        let mut track = mixer::mix(&media.key, media.duration, &project.sources, &clips)?;
        let path = self.path(&media.key, "xml");
        track.file_path = path.to_string_lossy().into_owned();
        tokio::fs::create_dir_all(&self.directory)
            .await
            .map_err(AppError::io)?;
        if media.cancel.is_cancelled() {
            return Err(cancelled());
        }
        atomic_write(&path, mixer::to_xml(&track).as_bytes()).await?;
        project.clips = clips;
        project.mixed = Some(track);
        self.save(&project, &media.cancel).await?;
        Ok(project.view(vec![]))
    }
    pub async fn export(&self, session: &str, path: PathBuf) -> AppResult<String> {
        let (media, _job) = self.begin(session).await?;
        let project = self.read(&media).await?;
        let track = project
            .mixed
            .ok_or_else(|| AppError::new("DANMAKU_EMPTY", "请先生成混合弹幕。"))?;
        atomic_write(&path, mixer::to_xml(&track).as_bytes()).await?;
        Ok(path.to_string_lossy().into_owned())
    }
}
#[derive(serde::Serialize)]
pub struct MatchResult {
    pub clips: Vec<Clip>,
    pub errors: Vec<String>,
}

async fn atomic_write(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let path = path.to_owned();
    let bytes = bytes.to_owned();
    tokio::task::spawn_blocking(move || {
        use std::io::Write;
        let parent = path
            .parent()
            .ok_or_else(|| AppError::new("DANMAKU_PATH", "文件路径无效。"))?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(AppError::io)?;
        file.write_all(&bytes).map_err(AppError::io)?;
        file.as_file().sync_all().map_err(AppError::io)?;
        file.persist(&path).map_err(AppError::io)?;
        Ok(())
    })
    .await
    .map_err(AppError::io)?
}

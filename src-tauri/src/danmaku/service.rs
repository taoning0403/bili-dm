use super::{
    bilibili::{cancelled, DanmakuProvider},
    engine::{MatchingEngine, TargetIndex},
    mixer,
    models::*,
    strategy::MatchOptions,
};
use crate::{
    core::{
        error::{AppError, AppResult},
        playback_service::{AnalysisMedia, PlaybackService},
    },
    player::probe::Progress,
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
    engine: MatchingEngine,
    directory: PathBuf,
    serial: Arc<Semaphore>,
    status: Arc<Mutex<JobStatus>>,
    cancellation: Mutex<Option<CancellationToken>>,
}
struct Job {
    _permit: OwnedSemaphorePermit,
    status: Arc<Mutex<JobStatus>>,
    cancel: CancellationToken,
    deadline: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Some(deadline) = &self.deadline {
            deadline.abort();
        }
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
            engine: MatchingEngine::new(resources),
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
            deadline: None,
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
        self.match_with_options(session, source_ids, MatchOptions::default())
            .await
    }
    pub async fn match_with_options(
        &self,
        session: &str,
        source_ids: Vec<String>,
        options: MatchOptions,
    ) -> AppResult<MatchResult> {
        options.validate()?;
        let (media, mut job) = self.begin(session).await?;
        job.limit(options.budget_seconds);
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
        let target = self
            .engine
            .prepare(
                &media,
                &options,
                self.progress("建立当前视频音画索引".into()),
            )
            .await?;
        let mut result = MatchResult {
            clips: vec![],
            errors: target.warnings.clone(),
        };
        for (index, source) in sources.iter().enumerate() {
            self.report(
                format!(
                    "匹配 {} / {}：{}",
                    index + 1,
                    sources.len(),
                    source.info.title
                ),
                index,
                sources.len(),
            );
            match self
                .match_one(&media, &source.info, &target, &options)
                .await
            {
                Ok(matched) => {
                    if result.clips.len() + matched.clips.len() > 500 {
                        result
                            .errors
                            .push("匹配区间总量达到 500 段上限，请减少所选来源。".into());
                        break;
                    }
                    result.clips.extend(matched.clips);
                    result.errors.extend(matched.warnings);
                }
                Err(e) if e.code == "CANCELLED" => return Err(e),
                Err(e) => result
                    .errors
                    .push(format!("{}：{}", source.info.title, e.message)),
            }
        }
        Ok(result)
    }
    async fn match_one(
        &self,
        media: &AnalysisMedia,
        source: &SourceInfo,
        target: &TargetIndex,
        options: &MatchOptions,
    ) -> AppResult<super::engine::AlignedSource> {
        let stream = self.provider.stream(source, &media.cancel).await?;
        self.engine
            .match_source(
                media,
                source,
                stream,
                target,
                options,
                self.progress(format!("分析：{}", source.title)),
            )
            .await
    }
    pub async fn search_and_match(
        &self,
        session: &str,
        query: String,
        page: u32,
        options: MatchOptions,
    ) -> AppResult<SearchResult> {
        options.validate()?;
        let (media, mut job) = self.begin(session).await?;
        job.limit(options.budget_seconds);
        let mut project = self.read(&media).await?;
        self.report("搜索 B 站视频、番剧与影视…", 0, 0);
        let discovery = self.provider.search(&query, page, &media.cancel).await?;
        let mut errors = discovery.warnings;
        let mut candidates = vec![];
        let mut clips = vec![];
        let mut seen = HashSet::new();
        let mut target = None;
        let episode = requested_episode(&query);
        let mut truncated = false;
        'hits: for hit in discovery.hits {
            if candidates.len() >= options.max_candidates {
                truncated = true;
                break;
            }
            let mut sources = match self.provider.videos(&hit.input, &media.cancel).await {
                Ok(s) => s,
                Err(e) if e.code == "CANCELLED" => return Err(e),
                Err(e) => {
                    candidates.push(SearchCandidate {
                        title: hit.title,
                        input: hit.input,
                        duration: None,
                        state: "unavailable".into(),
                        reason: e.message,
                        coverage: 0.0,
                        clip_count: 0,
                    });
                    continue;
                }
            };
            if let Some(episode) = episode {
                sources.sort_by_key(|s| s.page != episode);
                if sources
                    .iter()
                    .any(|s| s.episode_id.is_some() && s.page == episode)
                {
                    sources.retain(|s| s.episode_id.is_none() || s.page == episode);
                }
            }
            for source in sources {
                if candidates.len() >= options.max_candidates {
                    truncated = true;
                    break 'hits;
                }
                if !seen.insert(source.id.clone()) {
                    continue;
                }
                let input = source
                    .episode_id
                    .map(|id| format!("ep{id}"))
                    .unwrap_or_else(|| {
                        format!(
                            "https://www.bilibili.com/video/{}?p={}",
                            source.bvid, source.page
                        )
                    });
                let mut candidate = SearchCandidate {
                    title: source.title.clone(),
                    input,
                    duration: Some(source.duration),
                    state: "filtered".into(),
                    reason: String::new(),
                    coverage: 0.0,
                    clip_count: 0,
                };
                if !options.duration_allowed(source.duration, media.duration) {
                    candidate.reason = format!(
                        "时长 {:.1}s，低于当前视频的 {:.0}%",
                        source.duration,
                        options.min_duration_ratio * 100.0
                    );
                    candidates.push(candidate);
                    continue;
                }
                if target.is_none() {
                    target = Some(
                        self.engine
                            .prepare(
                                &media,
                                &options,
                                self.progress("建立当前视频音画索引".into()),
                            )
                            .await?,
                    );
                    if let Some(t) = &target {
                        errors.extend(t.warnings.clone());
                    }
                }
                let Some(index) = &target else {
                    continue;
                };
                match self.match_one(&media, &source, index, &options).await {
                    Ok(matched) => {
                        candidate.coverage = matched.coverage;
                        candidate.clip_count = matched.clips.len();
                        candidate.state = if matched.clips.iter().all(|c| !c.enabled) {
                            "review"
                        } else {
                            "matched"
                        }
                        .into();
                        candidate.reason = matched.warnings.join("；");
                        if project.clips.len() + clips.len() + matched.clips.len() > 500 {
                            candidate.state = "limit".into();
                            candidate.reason =
                                "区间总量将超过 500 段，请先减少现有区间或匹配来源。".into();
                            candidates.push(candidate);
                            continue;
                        }
                        // Only fetch comments for candidates which have alignment evidence.
                        let parsed =
                            match self.provider.comments(source.clone(), &media.cancel).await {
                                Ok(p) => p,
                                Err(e) if e.code == "CANCELLED" => return Err(e),
                                Err(e) => {
                                    candidate.state = "unavailable".into();
                                    candidate.reason = e.message;
                                    candidates.push(candidate);
                                    continue;
                                }
                            };
                        let count: usize = project
                            .sources
                            .iter()
                            .filter(|s| s.info.id != source.id)
                            .map(|s| s.comments.len())
                            .sum();
                        let old = project.sources.iter_mut().find(|s| s.info.id == source.id);
                        if count + parsed.comments.len() > MAX_COMMENTS
                            || (old.is_none() && project.sources.len() >= MAX_SOURCES)
                        {
                            candidate.state = "limit".into();
                            candidate.reason = "项目来源或弹幕数量达到上限。".into();
                        } else {
                            if let Some(old) =
                                project.sources.iter_mut().find(|s| s.info.id == source.id)
                            {
                                *old = parsed;
                            } else {
                                project.sources.push(parsed);
                            }
                            clips.extend(matched.clips);
                        }
                    }
                    Err(e) if e.code == "CANCELLED" => return Err(e),
                    Err(e) => {
                        candidate.state = if e.code == "MATCH_NOT_FOUND" {
                            "unmatched"
                        } else {
                            "unavailable"
                        }
                        .into();
                        candidate.reason = e.message;
                    }
                }
                candidates.push(candidate);
            }
        }
        if truncated {
            errors.push(format!(
                "本页已检查 {} 个候选；可增大候选上限重搜本页。",
                options.max_candidates
            ));
        }
        self.save(&project, &media.cancel).await?;
        Ok(SearchResult {
            workspace: project.view(vec![]),
            clips,
            candidates,
            errors,
            page,
            has_more: discovery.has_more,
        })
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
    pub async fn preview(
        &self,
        session: &str,
        source_id: &str,
        source_time: f64,
        target_time: f64,
    ) -> AppResult<super::engine::AlignmentPreview> {
        let (media, mut job) = self.begin(session).await?;
        job.limit(90);
        let project = self.read(&media).await?;
        let source = project
            .sources
            .iter()
            .find(|s| s.info.id == source_id)
            .ok_or_else(|| AppError::new("DANMAKU_SOURCE", "弹幕源不存在。"))?;
        if !source_time.is_finite()
            || !target_time.is_finite()
            || source_time < 0.0
            || source_time >= source.info.duration
            || target_time < 0.0
            || target_time >= media.duration
        {
            return Err(AppError::new("DANMAKU_RANGE", "预览位置不在视频范围内。"));
        }
        self.report("读取来源与当前视频的对应画面…", 0, 2);
        let stream = self.provider.stream(&source.info, &media.cancel).await?;
        self.engine
            .preview(&media, stream, source_time, target_time)
            .await
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

impl Job {
    fn limit(&mut self, seconds: u64) {
        let token = self.cancel.clone();
        self.deadline = Some(tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
            token.cancel();
        }));
    }
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCandidate {
    pub title: String,
    pub input: String,
    pub duration: Option<f64>,
    pub state: String,
    pub reason: String,
    pub coverage: f64,
    pub clip_count: usize,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub workspace: Workspace,
    pub clips: Vec<Clip>,
    pub candidates: Vec<SearchCandidate>,
    pub errors: Vec<String>,
    pub page: u32,
    pub has_more: bool,
}
pub fn requested_episode(query: &str) -> Option<u32> {
    regex::Regex::new(
        r"(?i)(?:第\s*|(?:^|[^a-z])(?:s\d{1,2})?e(?:p)?\s*)(\d{1,3})(?:\s*[集话話]|\b)",
    )
    .ok()?
    .captures(query)?
    .get(1)?
    .as_str()
    .parse()
    .ok()
}

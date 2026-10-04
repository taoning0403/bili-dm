use super::error::{AppError, AppResult};
use crate::{
    database::{
        models::{MediaRecord, PlaybackPreferences, PlaybackProgress, RepeatMode, TrackPreference},
        LibraryRepository,
    },
    media::{
        files::{classify, FileKind, MediaFile},
        playlist::{matching_subtitles, video_queue},
        stream_server::MediaServer,
    },
    player::{MediaTrack, PlayerBackend, PlayerControl, PlayerSnapshot, VideoViewport},
    torrent::{models::DownloadStats, TorrentEngine},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OnceCell, RwLock};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveMedia {
    pub session_id: String,
    pub title: String,
    pub source: &'static str,
    pub torrent_id: Option<String>,
    pub file_index: Option<usize>,
    pub local_path: Option<String>,
    pub resumed_from: f64,
}
impl ActiveMedia {
    pub fn path(&self) -> String {
        self.local_path.clone().unwrap_or_else(|| {
            format!(
                "torrent://{}/{}",
                self.torrent_id.as_deref().unwrap_or_default(),
                self.file_index.unwrap_or_default()
            )
        })
    }
    fn record(&self, duration: Option<f64>) -> MediaRecord {
        let path = self.path();
        MediaRecord {
            id: format!("{}:{path}", self.source),
            path,
            filename: self
                .title
                .rsplit('/')
                .next()
                .unwrap_or(&self.title)
                .to_owned(),
            duration,
            torrent_id: self.torrent_id.clone(),
            file_index: self.file_index,
        }
    }
}
#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub media: Option<ActiveMedia>,
    pub player: PlayerSnapshot,
    pub download: Option<DownloadStats>,
    pub queue: Vec<MediaFile>,
    pub queue_index: Option<usize>,
    pub preferences: PlaybackPreferences,
    pub warning: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueOptions {
    pub auto_next: bool,
    pub prefetch_next: bool,
    pub repeat: RepeatMode,
}

struct SessionState {
    analysis_cancel: CancellationToken,
    playable_source: Option<String>,
    media: Option<ActiveMedia>,
    queue: Vec<MediaFile>,
    subtitles: Vec<MediaFile>,
    mounted: HashSet<usize>,
    ready: bool,
    ended: bool,
    prefetch_attempted: bool,
    saved_at: Instant,
    preferences: Option<PlaybackPreferences>,
    warning: Option<String>,
}
impl Default for SessionState {
    fn default() -> Self {
        Self {
            analysis_cancel: CancellationToken::new(),
            playable_source: None,
            media: None,
            queue: vec![],
            subtitles: vec![],
            mounted: HashSet::new(),
            ready: false,
            ended: false,
            prefetch_attempted: false,
            saved_at: Instant::now(),
            preferences: None,
            warning: None,
        }
    }
}
pub struct PlaybackService {
    torrent: Arc<dyn TorrentEngine>,
    player: Arc<dyn PlayerBackend>,
    library: Arc<dyn LibraryRepository>,
    server: OnceCell<MediaServer>,
    session: Mutex<SessionState>,
    published: RwLock<PlaybackState>,
    opening: Mutex<CancellationToken>,
    lifetime: CancellationToken,
}
pub struct AnalysisMedia {
    pub key: String,
    pub source: String,
    pub duration: f64,
    pub cancel: CancellationToken,
}
impl PlaybackService {
    pub fn new(
        torrent: Arc<dyn TorrentEngine>,
        player: Arc<dyn PlayerBackend>,
        library: Arc<dyn LibraryRepository>,
    ) -> Self {
        Self {
            torrent,
            player,
            library,
            server: OnceCell::new(),
            session: Mutex::new(SessionState::default()),
            published: RwLock::new(PlaybackState::default()),
            opening: Mutex::new(CancellationToken::new()),
            lifetime: CancellationToken::new(),
        }
    }
    pub async fn monitor(self: Arc<Self>) {
        let service = self;
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! { _ = service.lifetime.cancelled() => break, _ = interval.tick() => {} }
            if let Err(error) = service.tick().await {
                service.published.write().await.warning = Some(error.to_string());
            }
        }
    }
    async fn begin_open(&self) -> CancellationToken {
        let mut opening = self.opening.lock().await;
        opening.cancel();
        *opening = CancellationToken::new();
        opening.clone()
    }
    async fn preferences(&self, state: &mut SessionState) -> AppResult<PlaybackPreferences> {
        if state.preferences.is_none() {
            state.preferences = Some(self.library.preferences().await?);
        }
        Ok(state.preferences.clone().unwrap_or_default())
    }
    pub async fn play_torrent(&self, id: &str, index: usize) -> AppResult<ActiveMedia> {
        let catalog = self.torrent.catalog(id).await?;
        if !catalog
            .files
            .iter()
            .any(|f| f.index == index && f.kind == FileKind::Video && f.size > 0)
        {
            return Err(AppError::new("INVALID_FILE", "请选择可播放的视频。"));
        }
        let cancel = self.begin_open().await;
        let mut state = self.session.lock().await;
        self.start_torrent(&mut state, id, index, &cancel, false)
            .await
    }
    async fn start_torrent(
        &self,
        state: &mut SessionState,
        id: &str,
        index: usize,
        cancel: &CancellationToken,
        restart: bool,
    ) -> AppResult<ActiveMedia> {
        if cancel.is_cancelled() {
            return Err(AppError::new("CANCELLED", "已取消打开视频。"));
        }
        let catalog = self.torrent.catalog(id).await?;
        self.stop_inner(state, false).await?;
        let result = async {
            let file = tokio::select! {
                _ = cancel.cancelled() => return Err(AppError::new("CANCELLED", "已取消打开视频。")),
                file = self.torrent.prepare_file(id, index) => file?,
            };
            let server = self
                .server
                .get_or_try_init(|| MediaServer::start(self.torrent.clone()))
                .await?;
            let url = server.source_url(id.to_owned(), file.clone()).await;
            let mut media = ActiveMedia {
                session_id: uuid::Uuid::new_v4().to_string(),
                title: file.path.clone(),
                source: "torrent",
                torrent_id: Some(id.to_owned()),
                file_index: Some(index),
                local_path: None,
                resumed_from: 0.0,
            };
            self.load_media(state, &mut media, &url, restart).await?;
            // Await native load completion before cleanup: spawn_blocking work
            // cannot be cancelled by dropping its future.
            if cancel.is_cancelled() { return Err(AppError::new("CANCELLED", "已取消打开视频。")); }
            state.queue = video_queue(&catalog.files);
            state.subtitles = matching_subtitles(&catalog.files, &file);
            Ok(media)
        }.await;
        if result.is_err() {
            let _ = self.stop_inner(state, true).await;
        }
        self.publish(state).await?;
        result
    }
    async fn load_media(
        &self,
        state: &mut SessionState,
        media: &mut ActiveMedia,
        source: &str,
        restart: bool,
    ) -> AppResult<()> {
        self.preferences(state).await?;
        if !restart {
            if let Some(progress) = self.library.progress(&media.path()).await? {
                if !progress.completed
                    && progress.position >= 3.0
                    && progress.duration - progress.position > 5.0
                {
                    media.resumed_from = progress.position;
                }
            }
        }
        // Persist identity before playback; a storage failure must not leave an
        // untracked download running. Store current before load for cleanup.
        self.library.save_media(media.record(None)).await?;
        state.media = Some(media.clone());
        state.playable_source = Some(source.to_owned());
        state.analysis_cancel = CancellationToken::new();
        self.player.load(source, media.resumed_from).await?;
        state.ready = false;
        state.ended = false;
        state.prefetch_attempted = false;
        state.mounted.clear();
        state.warning = None;
        state.saved_at = Instant::now();
        Ok(())
    }
    pub async fn play_local(&self, path: PathBuf) -> AppResult<ActiveMedia> {
        let path = validate_local(path, FileKind::Video).await?;
        let source = path
            .to_str()
            .ok_or_else(|| AppError::new("INVALID_PATH", "暂不支持非 UTF-8 文件路径。"))?
            .to_owned();
        let cancel = self.begin_open().await;
        let mut state = self.session.lock().await;
        if cancel.is_cancelled() {
            return Err(AppError::new("CANCELLED", "已取消打开视频。"));
        }
        self.stop_inner(&mut state, true).await?;
        let mut media = ActiveMedia {
            session_id: uuid::Uuid::new_v4().to_string(),
            title: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            source: "local",
            torrent_id: None,
            file_index: None,
            local_path: Some(source.clone()),
            resumed_from: 0.0,
        };
        let mut result = self
            .load_media(&mut state, &mut media, &source, false)
            .await;
        if cancel.is_cancelled() {
            result = Err(AppError::new("CANCELLED", "已取消打开视频。"));
        }
        if result.is_err() {
            let _ = self.stop_inner(&mut state, true).await;
        }
        self.publish(&state).await?;
        result.map(|_| media)
    }
    pub async fn add_subtitle(&self, path: PathBuf) -> AppResult<()> {
        let path = validate_local(path, FileKind::Subtitle).await?;
        let mut state = self.session.lock().await;
        if !self.player.snapshot().await?.loaded {
            return Err(AppError::new("NO_PLAYER", "请先等待视频加载完成。"));
        }
        self.player
            .add_subtitle(
                &path.to_string_lossy(),
                &path.file_name().unwrap_or_default().to_string_lossy(),
            )
            .await?;
        if let Some(track) = self
            .player
            .snapshot()
            .await?
            .tracks
            .iter()
            .rev()
            .find(|t| t.kind == "sub" && t.external)
        {
            self.player
                .control(PlayerControl::SubtitleTrack { id: track.id })
                .await?;
        }
        state.warning = None;
        self.publish(&state).await
    }
    pub async fn select_queue(&self, index: usize) -> AppResult<ActiveMedia> {
        let cancel = self.begin_open().await;
        let mut state = self.session.lock().await;
        let id = state
            .media
            .as_ref()
            .and_then(|m| m.torrent_id.clone())
            .ok_or_else(|| AppError::new("NO_QUEUE", "当前没有磁力播放队列。"))?;
        let file_index = state
            .queue
            .get(index)
            .ok_or_else(|| AppError::new("NO_QUEUE", "队列索引无效。"))?
            .index;
        self.start_torrent(&mut state, &id, file_index, &cancel, false)
            .await
    }
    pub async fn queue_options(&self, options: QueueOptions) -> AppResult<()> {
        let mut state = self.session.lock().await;
        let mut prefs = self.preferences(&mut state).await?;
        if !options.prefetch_next || options.repeat != prefs.repeat {
            if let Some(id) = state.media.as_ref().and_then(|m| m.torrent_id.as_deref()) {
                // A finished/stopped download is already inactive.
                if self.torrent.download_stats().await.is_some() {
                    self.torrent.prefetch(id, None).await?;
                }
            }
        }
        prefs.auto_next = options.auto_next;
        prefs.prefetch_next = options.prefetch_next;
        prefs.repeat = options.repeat;
        self.library.save_preferences(prefs.clone()).await?;
        state.preferences = Some(prefs);
        state.prefetch_attempted = false;
        self.publish(&state).await
    }
    pub async fn control(&self, control: PlayerControl) -> AppResult<()> {
        if matches!(control, PlayerControl::Stop) {
            self.opening.lock().await.cancel();
        }
        let mut state = self.session.lock().await;
        if matches!(control, PlayerControl::Stop) {
            let result = self.stop_inner(&mut state, true).await;
            self.publish(&state).await?;
            return result;
        }
        let before = self.player.snapshot().await?;
        let choice = match &control {
            PlayerControl::AudioTrack { id } => Some(("audio", *id)),
            PlayerControl::SubtitleTrack { id } => Some(("sub", *id)),
            _ => None,
        };
        if let Some((kind, id)) = choice {
            if id != 0 && !before.tracks.iter().any(|t| t.id == id && t.kind == kind) {
                return Err(AppError::new("INVALID_TRACK", "该音轨或字幕已不可用。"));
            }
        }
        self.player.control(control.clone()).await?;
        let mut prefs = self.preferences(&mut state).await?;
        let changed = match control {
            PlayerControl::Volume { volume } => {
                prefs.volume = volume;
                true
            }
            PlayerControl::Mute { muted } => {
                prefs.muted = muted;
                true
            }
            PlayerControl::Speed { speed } => {
                prefs.speed = speed;
                true
            }
            PlayerControl::AudioTrack { id } => {
                prefs.audio = Some(track_choice(&before.tracks, "audio", id));
                true
            }
            PlayerControl::SubtitleTrack { id } => {
                prefs.subtitle = Some(track_choice(&before.tracks, "sub", id));
                true
            }
            _ => false,
        };
        if changed {
            self.library.save_preferences(prefs.clone()).await?;
            state.preferences = Some(prefs);
        }
        self.persist(&mut state).await?;
        self.publish(&state).await
    }
    pub async fn viewport(&self, viewport: VideoViewport) -> AppResult<()> {
        self.player.viewport(viewport).await
    }
    async fn persist(&self, state: &mut SessionState) -> AppResult<()> {
        let Some(media) = &state.media else {
            return Ok(());
        };
        let snapshot = self.player.snapshot().await?;
        if snapshot.loaded
            && snapshot.duration.is_finite()
            && snapshot.duration > 0.0
            && snapshot.position.is_finite()
        {
            self.library
                .save_media(media.record(Some(snapshot.duration)))
                .await?;
            self.library
                .save_progress(PlaybackProgress {
                    path: media.path(),
                    position: snapshot.position.clamp(0.0, snapshot.duration),
                    duration: snapshot.duration,
                    completed: snapshot.ended,
                })
                .await?;
            state.saved_at = Instant::now();
        }
        Ok(())
    }
    async fn stop_inner(&self, state: &mut SessionState, clear_queue: bool) -> AppResult<()> {
        state.analysis_cancel.cancel();
        state.playable_source = None;
        let saved = self.persist(state).await;
        // Cancel readers before pausing the engine, including blocked HTTP bodies.
        if let Some(server) = self.server.get() {
            server.clear().await;
        }
        if self.player.control(PlayerControl::Stop).await.is_err() {
            self.player.shutdown().await;
        }
        let paused = self.torrent.pause_download().await;
        let previous = state.media.take();
        state.ready = false;
        state.subtitles.clear();
        state.mounted.clear();
        if clear_queue {
            state.queue.clear();
            state.warning = None;
        }
        let stored = match previous.and_then(|m| m.torrent_id) {
            Some(id) => self.library.pause_task(&id).await,
            None => Ok(()),
        };
        saved?;
        paused?;
        stored
    }
    pub async fn analysis_media(&self, session_id: &str) -> AppResult<AnalysisMedia> {
        let state = self.session.lock().await;
        let media = state
            .media
            .as_ref()
            .filter(|m| m.session_id == session_id)
            .ok_or_else(|| AppError::new("MEDIA_CHANGED", "当前视频已切换，请重新操作弹幕。"))?;
        let player = self.player.snapshot().await?;
        if !player.loaded || !player.duration.is_finite() || player.duration <= 0.0 {
            return Err(AppError::new(
                "NO_PLAYER",
                "请先打开视频并等待时长加载完成。",
            ));
        }
        Ok(AnalysisMedia {
            key: media.path(),
            duration: player.duration,
            source: state
                .playable_source
                .clone()
                .ok_or_else(|| AppError::new("NO_PLAYER", "视频源尚未就绪。"))?,
            cancel: state.analysis_cancel.child_token(),
        })
    }
    pub async fn tick(&self) -> AppResult<()> {
        let Ok(mut state) = self.session.try_lock() else {
            return Ok(());
        };
        self.preferences(&mut state).await?;
        let snapshot = self.player.snapshot().await?;
        if state.media.is_none() {
            return self.publish(&state).await;
        }
        if let Some(error) = snapshot.error.as_ref() {
            if let Some(server) = self.server.get() {
                server.clear().await;
            }
            self.torrent.pause_download().await?;
            if let Some(id) = state.media.as_ref().and_then(|m| m.torrent_id.as_deref()) {
                self.library.pause_task(id).await?;
            }
            state.warning = Some(format!("播放失败：{error}"));
            return self.publish(&state).await;
        }
        if snapshot.loaded {
            if state.ended && !snapshot.ended {
                state.ended = false;
                state.prefetch_attempted = false;
            }
            if !state.ready {
                let prefs = self.preferences(&mut state).await?;
                self.player
                    .control(PlayerControl::Volume {
                        volume: prefs.volume,
                    })
                    .await?;
                self.player
                    .control(PlayerControl::Mute { muted: prefs.muted })
                    .await?;
                self.player
                    .control(PlayerControl::Speed { speed: prefs.speed })
                    .await?;
                self.restore_tracks(&prefs, &snapshot.tracks).await?;
                state.ready = true;
                self.persist(&mut state).await?;
            }
            if let Some(id) = state.media.as_ref().and_then(|m| m.torrent_id.clone()) {
                for file in state.subtitles.clone() {
                    if state.mounted.contains(&file.index) {
                        continue;
                    }
                    if let Some(path) = self.torrent.subtitle_path(&id, file.index).await? {
                        match self
                            .player
                            .add_subtitle(&path.to_string_lossy(), &file.path)
                            .await
                        {
                            Ok(()) => {}
                            Err(error) => state.warning = Some(format!("字幕加载失败：{error}")),
                        }
                        state.mounted.insert(file.index);
                        let tracks = self.player.snapshot().await?.tracks;
                        if state
                            .preferences
                            .as_ref()
                            .and_then(|p| p.subtitle.as_ref())
                            .is_none()
                            && !tracks.iter().any(|t| t.kind == "sub" && t.selected)
                        {
                            if let Some(track) =
                                tracks.iter().find(|t| t.kind == "sub" && t.external)
                            {
                                self.player
                                    .control(PlayerControl::SubtitleTrack { id: track.id })
                                    .await?;
                            }
                        }
                        self.restore_tracks(
                            &state.preferences.clone().unwrap_or_default(),
                            &self.player.snapshot().await?.tracks,
                        )
                        .await?;
                    }
                }
            }
            if !state.ended && state.saved_at.elapsed() >= Duration::from_secs(5) {
                self.persist(&mut state).await?;
            }
            let prefs = state.preferences.clone().unwrap_or_default();
            let next = next_index(&state.queue, state.media.as_ref(), prefs.repeat);
            if snapshot.ended && !state.ended {
                self.persist(&mut state).await?;
                state.ended = true;
                if prefs.auto_next || prefs.repeat == RepeatMode::One {
                    if let Some(next) = next {
                        let id = state.media.as_ref().and_then(|m| m.torrent_id.clone());
                        if let Some(id) = id {
                            let cancel = self.begin_open().await;
                            let file = state.queue[next].index;
                            self.start_torrent(&mut state, &id, file, &cancel, true)
                                .await?;
                            return Ok(());
                        }
                    }
                    if prefs.repeat == RepeatMode::One {
                        self.player
                            .control(PlayerControl::Seek { seconds: 0.0 })
                            .await?;
                        self.player
                            .control(PlayerControl::Pause { paused: false })
                            .await?;
                        state.ended = false;
                    }
                }
                if state.ended {
                    self.torrent.suspend_download().await?;
                    if let Some(id) = state.media.as_ref().and_then(|m| m.torrent_id.as_deref()) {
                        self.library.pause_task(id).await?;
                    }
                    state.subtitles.clear();
                }
            } else if !state.ended
                && prefs.prefetch_next
                && prefs.repeat != RepeatMode::One
                && !state.prefetch_attempted
            {
                if let (Some(next), Some(stats)) = (next, self.torrent.download_stats().await) {
                    if stats.total > 0 && stats.downloaded >= stats.total {
                        state.prefetch_attempted = true;
                        if let Err(error) = self
                            .torrent
                            .prefetch(&stats.torrent_id, Some(state.queue[next].index))
                            .await
                        {
                            state.warning = Some(format!("下一集预取失败：{error}"));
                        }
                    }
                }
            }
        }
        self.publish(&state).await
    }
    async fn restore_tracks(
        &self,
        prefs: &PlaybackPreferences,
        tracks: &[MediaTrack],
    ) -> AppResult<()> {
        for (kind, choice) in [("audio", &prefs.audio), ("sub", &prefs.subtitle)] {
            if let Some(choice) = choice {
                let id = if choice.disabled {
                    Some(0)
                } else {
                    preferred_track(tracks, kind, choice)
                };
                if let Some(id) = id {
                    self.player
                        .control(if kind == "audio" {
                            PlayerControl::AudioTrack { id }
                        } else {
                            PlayerControl::SubtitleTrack { id }
                        })
                        .await?;
                }
            }
        }
        Ok(())
    }
    async fn publish(&self, state: &SessionState) -> AppResult<()> {
        *self.published.write().await = PlaybackState {
            media: state.media.clone(),
            player: self.player.snapshot().await?,
            download: self.torrent.download_stats().await,
            queue: state.queue.clone(),
            queue_index: state.media.as_ref().and_then(|m| {
                state
                    .queue
                    .iter()
                    .position(|f| Some(f.index) == m.file_index)
            }),
            preferences: state.preferences.clone().unwrap_or_default(),
            warning: state.warning.clone(),
        };
        Ok(())
    }
    pub async fn snapshot(&self) -> AppResult<PlaybackState> {
        Ok(self.published.read().await.clone())
    }
    pub async fn shutdown(&self) {
        self.lifetime.cancel();
        self.opening.lock().await.cancel();
        let mut state = self.session.lock().await;
        if let Err(error) = self.stop_inner(&mut state, true).await {
            eprintln!("关闭播放任务：{error}");
        }
        self.player.shutdown().await;
        self.torrent.shutdown().await;
    }
}
fn track_choice(tracks: &[MediaTrack], kind: &str, id: i64) -> TrackPreference {
    let t = tracks.iter().find(|t| t.kind == kind && t.id == id);
    TrackPreference {
        disabled: id == 0,
        language: t.map(|t| t.language.clone()).unwrap_or_default(),
        title: t.map(|t| t.title.clone()).unwrap_or_default(),
        codec: t.map(|t| t.codec.clone()).unwrap_or_default(),
    }
}
pub fn preferred_track(tracks: &[MediaTrack], kind: &str, choice: &TrackPreference) -> Option<i64> {
    tracks
        .iter()
        .filter(|t| t.kind == kind)
        .filter_map(|t| {
            let score = usize::from(!choice.language.is_empty() && t.language == choice.language)
                * 4
                + usize::from(!choice.title.is_empty() && t.title == choice.title) * 2
                + usize::from(!choice.codec.is_empty() && t.codec == choice.codec);
            (score > 0).then_some((score, t.id))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, id)| id)
}
fn next_index(
    queue: &[MediaFile],
    media: Option<&ActiveMedia>,
    repeat: RepeatMode,
) -> Option<usize> {
    let current = media.and_then(|m| queue.iter().position(|f| Some(f.index) == m.file_index))?;
    if repeat == RepeatMode::One {
        Some(current)
    } else if current + 1 < queue.len() {
        Some(current + 1)
    } else if repeat == RepeatMode::All {
        Some(0)
    } else {
        None
    }
}
async fn validate_local(path: PathBuf, kind: FileKind) -> AppResult<PathBuf> {
    let path = tokio::fs::canonicalize(path).await.map_err(AppError::io)?;
    let metadata = tokio::fs::metadata(&path).await.map_err(AppError::io)?;
    if !metadata.is_file() || metadata.len() == 0 || classify(&path.to_string_lossy()) != kind {
        return Err(AppError::new(
            "INVALID_FILE",
            "请选择受支持的非空媒体文件。",
        ));
    }
    if kind == FileKind::Subtitle && metadata.len() > 20 * 1024 * 1024 {
        return Err(AppError::new("SUBTITLE_SIZE", "字幕文件不能超过 20 MiB。"));
    }
    Ok(path)
}

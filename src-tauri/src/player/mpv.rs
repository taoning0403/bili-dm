use super::{
    executable::find_mpv, ipc::MpvConnection, PlayerBackend, PlayerControl, PlayerSnapshot,
};
use crate::core::error::{AppError, AppResult};
use async_trait::async_trait;
use serde_json::json;
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    process::{Child, Command},
    sync::Mutex,
};

struct RunningPlayer {
    child: Child,
    connection: MpvConnection,
    _socket_dir: tempfile::TempDir,
}

pub struct MpvBackend {
    resource_dir: PathBuf,
    running: Mutex<Option<RunningPlayer>>,
}

impl MpvBackend {
    pub fn new(resource_dir: PathBuf) -> Self {
        Self {
            resource_dir,
            running: Mutex::new(None),
        }
    }

    async fn start(&self) -> AppResult<RunningPlayer> {
        let executable = find_mpv(&self.resource_dir)?;
        let socket_dir = tempfile::Builder::new()
            .prefix("bili-mpv-")
            .tempdir()
            .map_err(AppError::io)?;
        #[cfg(unix)]
        let endpoint = socket_dir.path().join("ipc");
        #[cfg(windows)]
        let endpoint = PathBuf::from(format!(r"\\.\pipe\bili-dm-{}", uuid::Uuid::new_v4()));
        let mut command = Command::new(executable);
        command
            .args([
                "--no-config",
                "--idle=yes",
                "--force-window=yes",
                "--keep-open=yes",
                "--osc=yes",
                "--terminal=no",
                "--ytdl=no",
                "--title=Bili DM · 视频",
                "--cache=yes",
                "--cache-secs=20",
                "--demuxer-max-bytes=64MiB",
                "--network-timeout=120",
            ])
            .arg(format!("--input-ipc-server={}", endpoint.to_string_lossy()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command
            .spawn()
            .map_err(|error| AppError::new("MPV_START", format!("无法启动 mpv：{error}")))?;
        let started = std::time::Instant::now();
        loop {
            if let Some(status) = child.try_wait().map_err(AppError::io)? {
                return Err(AppError::new(
                    "MPV_START",
                    format!("mpv 启动后退出：{status}"),
                ));
            }
            if let Ok(connection) = MpvConnection::connect(&endpoint).await {
                return Ok(RunningPlayer {
                    child,
                    connection,
                    _socket_dir: socket_dir,
                });
            }
            if started.elapsed() > Duration::from_secs(10) {
                let _ = child.kill().await;
                return Err(AppError::new(
                    "MPV_START",
                    "10 秒内无法连接 mpv，请检查安装。",
                ));
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    }
}

#[async_trait]
impl PlayerBackend for MpvBackend {
    async fn load(&self, source: &str) -> AppResult<()> {
        let mut guard = self.running.lock().await;
        if let Some(running) = guard.as_mut() {
            if running.child.try_wait().map_err(AppError::io)?.is_some() {
                *guard = None;
            }
        }
        if guard.is_none() {
            *guard = Some(self.start().await?);
        }
        let running = guard
            .as_mut()
            .ok_or_else(|| AppError::new("MPV_START", "播放器未就绪。"))?;
        running.connection.last_error = None;
        running
            .connection
            .request(json!(["set_property", "pause", false]))
            .await?;
        running
            .connection
            .request(json!(["loadfile", source, "replace"]))
            .await?;
        Ok(())
    }

    async fn control(&self, control: PlayerControl) -> AppResult<()> {
        let command = match control {
            PlayerControl::Pause { paused } => json!(["set_property", "pause", paused]),
            PlayerControl::Seek { seconds } if seconds.is_finite() && seconds >= 0.0 => {
                json!(["seek", seconds, "absolute+exact"])
            }
            PlayerControl::Volume { volume }
                if volume.is_finite() && (0.0..=100.0).contains(&volume) =>
            {
                json!(["set_property", "volume", volume])
            }
            PlayerControl::Stop => json!(["stop"]),
            _ => {
                return Err(AppError::new(
                    "INVALID_CONTROL",
                    "播放位置或音量超出有效范围。",
                ))
            }
        };
        let mut guard = self.running.lock().await;
        if let Some(running) = guard.as_mut() {
            running.connection.request(command).await?;
        } else if !matches!(control, PlayerControl::Stop) {
            return Err(AppError::new("NO_PLAYER", "请先打开视频。"));
        }
        Ok(())
    }

    async fn snapshot(&self) -> AppResult<PlayerSnapshot> {
        let mut guard = self.running.lock().await;
        let Some(running) = guard.as_mut() else {
            return Ok(PlayerSnapshot::default());
        };
        if running.child.try_wait().map_err(AppError::io)?.is_some() {
            *guard = None;
            return Ok(PlayerSnapshot::default());
        }
        let ipc = &mut running.connection;
        let idle = ipc.property("idle-active").await?.as_bool().unwrap_or(true);
        let position = ipc.property("time-pos").await?.as_f64().unwrap_or(0.0);
        let duration = ipc.property("duration").await?.as_f64().unwrap_or(0.0);
        let paused = ipc.property("pause").await?.as_bool().unwrap_or(false);
        let volume = ipc.property("volume").await?.as_f64().unwrap_or(100.0);
        let buffering = ipc
            .property("paused-for-cache")
            .await?
            .as_bool()
            .unwrap_or(false);
        let ended = ipc
            .property("eof-reached")
            .await?
            .as_bool()
            .unwrap_or(false);
        Ok(PlayerSnapshot {
            running: true,
            loaded: !idle && duration > 0.0,
            paused,
            position,
            duration,
            volume,
            buffering,
            ended,
            error: ipc.last_error.clone(),
        })
    }

    async fn shutdown(&self) {
        if let Some(mut running) = self.running.lock().await.take() {
            let _ = running.connection.request(json!(["quit"])).await;
            if tokio::time::timeout(Duration::from_secs(2), running.child.wait())
                .await
                .is_err()
            {
                let _ = running.child.kill().await;
            }
        }
    }
}

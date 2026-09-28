use crate::core::error::{AppError, AppResult};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

pub trait IpcStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> IpcStream for T {}

pub struct MpvConnection {
    stream: BufReader<Box<dyn IpcStream>>,
    request_id: u64,
    pub last_error: Option<String>,
}

impl MpvConnection {
    pub async fn connect(endpoint: &Path) -> std::io::Result<Self> {
        #[cfg(unix)]
        let stream: Box<dyn IpcStream> = Box::new(tokio::net::UnixStream::connect(endpoint).await?);
        #[cfg(windows)]
        let stream: Box<dyn IpcStream> =
            Box::new(tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint)?);
        Ok(Self {
            stream: BufReader::new(stream),
            request_id: 0,
            last_error: None,
        })
    }

    pub async fn request(&mut self, command: Value) -> AppResult<Value> {
        tokio::time::timeout(Duration::from_secs(4), self.request_inner(command))
            .await
            .map_err(|_| AppError::new("MPV_TIMEOUT", "播放器响应超时，请停止后重新打开视频。"))?
    }

    async fn request_inner(&mut self, command: Value) -> AppResult<Value> {
        self.request_id = self.request_id.wrapping_add(1);
        let mut payload = json!({ "command": command, "request_id": self.request_id }).to_string();
        payload.push('\n');
        self.stream
            .get_mut()
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| AppError::new("MPV_DISCONNECTED", e.to_string()))?;
        loop {
            let mut line = String::new();
            let length = self
                .stream
                .read_line(&mut line)
                .await
                .map_err(|e| AppError::new("MPV_DISCONNECTED", e.to_string()))?;
            if length == 0 {
                return Err(AppError::new("MPV_DISCONNECTED", "播放器窗口已关闭。"));
            }
            let message: Value = serde_json::from_str(&line)
                .map_err(|_| AppError::new("MPV_PROTOCOL", "播放器返回了无效数据。"))?;
            if message["event"] == "end-file" && message["reason"] == "error" {
                self.last_error = Some(
                    message["file_error"]
                        .as_str()
                        .unwrap_or("视频加载失败，资源可能暂时无可用节点或无法解码。")
                        .to_owned(),
                );
            }
            if message["request_id"].as_u64() == Some(self.request_id) {
                return if message["error"] == "success" {
                    Ok(message["data"].clone())
                } else {
                    Err(AppError::new(
                        "MPV_COMMAND",
                        message["error"]
                            .as_str()
                            .unwrap_or("播放器命令失败")
                            .to_owned(),
                    ))
                };
            }
        }
    }

    pub async fn property(&mut self, name: &str) -> AppResult<Value> {
        match self.request(json!(["get_property", name])).await {
            Err(error)
                if error.code == "MPV_COMMAND" && error.message == "property unavailable" =>
            {
                Ok(Value::Null)
            }
            result => result,
        }
    }
}

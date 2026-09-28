//! Private loopback byte transport for mpv. No task API or arbitrary file access.
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::{io::SeekFrom, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt},
    net::TcpListener,
    sync::RwLock,
};
use tokio_util::{io::ReaderStream, sync::CancellationToken};
use uuid::Uuid;

use super::{files::MediaFile, range::byte_range};
use crate::{
    core::error::{AppError, AppResult},
    torrent::TorrentEngine,
};

#[derive(Clone)]
struct Source {
    token: String,
    torrent_id: String,
    file: MediaFile,
}

#[derive(Clone)]
struct StreamState {
    engine: Arc<dyn TorrentEngine>,
    source: Arc<RwLock<Option<Source>>>,
}

pub struct MediaServer {
    origin: String,
    state: StreamState,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl MediaServer {
    pub async fn start(engine: Arc<dyn TorrentEngine>) -> AppResult<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(AppError::io)?;
        let origin = format!("http://{}", listener.local_addr().map_err(AppError::io)?);
        let state = StreamState {
            engine,
            source: Arc::new(RwLock::new(None)),
        };
        let router = Router::new()
            .route("/stream/{token}", get(stream_file))
            .with_state(state.clone());
        let stop = CancellationToken::new();
        let signal = stop.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(signal.cancelled_owned())
                .await
            {
                eprintln!("Media transport stopped: {error}");
            }
        });
        Ok(Self {
            origin,
            state,
            stop,
            task,
        })
    }

    pub async fn source_url(&self, torrent_id: String, file: MediaFile) -> String {
        let token = Uuid::new_v4().to_string();
        let url = format!("{}/stream/{token}", self.origin);
        *self.state.source.write().await = Some(Source {
            token,
            torrent_id,
            file,
        });
        url
    }

    pub async fn clear(&self) {
        *self.state.source.write().await = None;
    }
}

impl Drop for MediaServer {
    fn drop(&mut self) {
        self.stop.cancel();
        self.task.abort();
    }
}

async fn stream_file(
    State(state): State<StreamState>,
    Path(token): Path<String>,
    headers: HeaderMap,
    method: Method,
) -> Response {
    let source = state.source.read().await.clone();
    let Some(source) = source.filter(|source| source.token == token) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let range = match headers
        .get(header::RANGE)
        .map(|value| value.to_str())
        .transpose()
    {
        Ok(range) => byte_range(range, source.file.size),
        Err(_) => Err("invalid header"),
    };
    let (start, length, partial) = match range {
        Ok(range) => range,
        Err(_) => {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(
                    header::CONTENT_RANGE,
                    format!("bytes */{}", source.file.size),
                )],
            )
                .into_response()
        }
    };
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        let mut reader = match state
            .engine
            .open_file(&source.torrent_id, source.file.index)
            .await
        {
            Ok(reader) => reader,
            Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        };
        if reader.seek(SeekFrom::Start(start)).await.is_err() {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        Body::from_stream(ReaderStream::with_capacity(reader.take(length), 256 * 1024))
    };
    let mut response = Response::new(body);
    *response.status_mut() = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let headers = response.headers_mut();
    headers.insert(
        header::ACCEPT_RANGES,
        axum::http::HeaderValue::from_static("bytes"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    if let Ok(value) = length.to_string().parse() {
        headers.insert(header::CONTENT_LENGTH, value);
    }
    if partial {
        if let Ok(value) = format!(
            "bytes {}-{}/{}",
            start,
            start + length - 1,
            source.file.size
        )
        .parse()
        {
            headers.insert(header::CONTENT_RANGE, value);
        }
    }
    response
}

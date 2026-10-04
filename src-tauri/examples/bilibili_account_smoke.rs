//! Public API checks only. Never prints login keys, cookies, signed URLs or QR pixels.
use bili_dm_lib::{
    bilibili::{auth::AuthService, session::BilibiliClient},
    danmaku::bilibili::{BilibiliProvider, DanmakuProvider},
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args().nth(1).unwrap_or("qr".into());
    let session = Arc::new(BilibiliClient::new(None)?);
    let auth = AuthService::new(session.clone());
    if arg == "qr" {
        let qr = auth.start().await?;
        println!("QR generated: {}x{}", qr.pixels.len(), qr.pixels.len());
        let result = auth.poll(&qr.ticket).await?;
        println!("poll status: {}", result.state);
        auth.cancel_login().await;
        return Ok(());
    }
    let provider = BilibiliProvider::with_session(session)?;
    let cancel = CancellationToken::new();
    if arg.starts_with("ep") || arg.starts_with("ss") {
        let sources = provider.videos(&arg, &cancel).await?;
        println!("episodes={} first={}", sources.len(), sources[0].title);
        match provider.stream(&sources[0], &cancel).await {
            Ok(s) => println!(
                "full stream duration={} audio={}",
                s.duration,
                s.audio_url.is_some()
            ),
            Err(e) => println!("stream status={}: {}", e.code, e.message),
        };
    } else {
        let results = provider.search(&arg, 1, &cancel).await?;
        println!(
            "hits={} hasMore={} warnings={:?}",
            results.hits.len(),
            results.has_more,
            results.warnings
        );
        for hit in results.hits.iter().take(5) {
            println!("{} {} {}", hit.kind, hit.input, hit.title);
        }
    }
    Ok(())
}

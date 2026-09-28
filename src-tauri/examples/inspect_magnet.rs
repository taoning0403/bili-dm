//! Read-only metadata smoke test. Pass a magnet as the only argument.
use bili_dm_lib::torrent::{rqbit::RqbitEngine, TorrentEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let magnet = std::env::args()
        .nth(1)
        .ok_or("usage: inspect_magnet 'magnet:?...'")?;
    let cache = tempfile::tempdir()?;
    let engine = RqbitEngine::new(cache.path().to_owned());
    let result = engine.resolve(&magnet).await;
    engine.shutdown().await;
    println!("{}", serde_json::to_string_pretty(&result?)?);
    Ok(())
}

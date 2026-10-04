//! Real libmpv frame analysis and optional public provider smoke (no user DB).
use bili_dm_lib::{
    danmaku::{
        bilibili::{BilibiliProvider, DanmakuProvider, REFERER, USER_AGENT},
        matching,
    },
    player::probe::{FrameProbe, ProbeInput},
};
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let probe = FrameProbe::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let cancel = CancellationToken::new();
    if args.first().is_some_and(|s| s.starts_with("BV")) {
        let provider = BilibiliProvider::new()?;
        let sources = provider.videos(&args[0], &cancel).await?;
        let source = sources.first().ok_or("no video")?;
        let parsed = provider.comments(source.clone(), &cancel).await?;
        println!(
            "PASS metadata: {} P{} duration={} comments={} warnings={:?}",
            source.bvid,
            source.page,
            source.duration,
            parsed.comments.len(),
            parsed.info.warnings
        );
        let stream = provider.stream(source, &cancel).await?;
        let (duration, frames) = probe
            .sample(
                ProbeInput {
                    source: stream.url,
                    referer: Some(REFERER.into()),
                    user_agent: Some(USER_AGENT.into()),
                },
                vec![1.0, stream.duration / 2.0, stream.duration - 3.0],
                cancel,
                Arc::new(|_, _| {}),
            )
            .await?;
        println!(
            "PASS remote video decode: duration={duration:.3}, frames={}, information={:?}",
            frames.len(),
            frames.iter().map(|f| f.information).collect::<Vec<_>>()
        );
        return Ok(());
    }
    if args.len() != 2 {
        return Err("usage: danmaku_smoke <target> <source> OR <BV>".into());
    }
    let input = |source: &str| ProbeInput {
        source: source.into(),
        referer: None,
        user_agent: None,
    };
    let (td, _) = probe
        .sample(
            input(&args[0]),
            vec![0.2],
            cancel.clone(),
            Arc::new(|_, _| {}),
        )
        .await?;
    let (sd, _) = probe
        .sample(
            input(&args[1]),
            vec![0.2],
            cancel.clone(),
            Arc::new(|_, _| {}),
        )
        .await?;
    let (_, target) = probe
        .sample(
            input(&args[0]),
            matching::sample_times(td, 1.0),
            cancel.clone(),
            Arc::new(|_, _| {}),
        )
        .await?;
    let (_, source) = probe
        .sample(
            input(&args[1]),
            matching::sample_times(sd, 1.0),
            cancel.clone(),
            Arc::new(|_, _| {}),
        )
        .await?;
    let candidates = matching::candidates(&source, &target, sd, td, 1.0);
    println!("Candidate offsets: {candidates:?}");
    for offset in candidates {
        let (_, anchors) = probe
            .sample(
                input(&args[1]),
                matching::anchor_times(offset, sd, td),
                cancel.clone(),
                Arc::new(|_, _| {}),
            )
            .await?;
        let mut times = vec![];
        for anchor in &anchors {
            for i in -6..=6 {
                let time = anchor.time + offset + i as f64 * 0.25;
                if time > 0.0 && time < td - 0.1 {
                    times.push(time);
                }
            }
        }
        times.sort_by(f64::total_cmp);
        times.dedup();
        let (_, fine) = probe
            .sample(input(&args[0]), times, cancel.clone(), Arc::new(|_, _| {}))
            .await?;
        if let Some(alignment) = matching::refine(&anchors, &fine, offset, 1.5, sd, td) {
            println!("PASS decoded alignment: {alignment:?}");
            return Ok(());
        }
    }
    Err("no verified match".into())
}

//! Decode controlled audiovisual fixtures using the bundled native runtime.
use bili_dm_lib::{
    core::playback_service::AnalysisMedia,
    danmaku::{
        bilibili::VideoStream,
        engine::MatchingEngine,
        models::SourceInfo,
        strategy::{EvidenceMode, MatchOptions},
    },
    player::probe::{FrameProbe, ProbeInput},
};
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: alignment_smoke target source [audio|video|hybrid]".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let probe = FrameProbe::new(root.clone());
    let cancel = CancellationToken::new();
    let input = |p: &str| ProbeInput {
        source: p.into(),
        referer: None,
        user_agent: None,
    };
    let td = probe
        .sample(
            input(&args[0]),
            vec![0.2],
            cancel.clone(),
            Arc::new(|_, _| {}),
        )
        .await?
        .0;
    let sd = probe
        .sample(
            input(&args[1]),
            vec![0.2],
            cancel.clone(),
            Arc::new(|_, _| {}),
        )
        .await?
        .0;
    let media = AnalysisMedia {
        key: "test-fixture".into(),
        source: args[0].clone(),
        duration: td,
        cancel,
        audio_track_id: None,
    };
    let mode = match args.get(2).map(String::as_str) {
        Some("audio") => EvidenceMode::Audio,
        Some("video") => EvidenceMode::Video,
        _ => EvidenceMode::Hybrid,
    };
    let options = MatchOptions {
        mode,
        allow_edits: true,
        allow_redraw: true,
        min_duration_ratio: 0.5,
        min_coverage: 0.5,
        sample_step: 1.0,
        min_segment: 2.0,
        ..Default::default()
    };
    let engine = MatchingEngine::new(root);
    let index = engine
        .prepare(&media, &options, Arc::new(|_, _| {}))
        .await?;
    let source = SourceInfo {
        id: "fixture".into(),
        bvid: String::new(),
        cid: 1,
        page: 1,
        title: "Controlled source".into(),
        duration: sd,
        comment_count: 0,
        warnings: vec![],
        episode_id: None,
    };
    let result = engine
        .match_source(
            &media,
            &source,
            VideoStream {
                url: args[1].clone(),
                audio_url: Some(args[1].clone()),
                duration: sd,
            },
            &index,
            &options,
            Arc::new(|_, _| {}),
        )
        .await?;
    println!(
        "coverage={:.3} warnings={:?}",
        result.coverage, result.warnings
    );
    for clip in result.clips {
        println!(
            "{:.3}..{:.3} => {:.3}..{:.3} {} enabled={}",
            clip.source_start,
            clip.source_end,
            clip.target_start,
            clip.target_end,
            clip.evidence.unwrap_or_default(),
            clip.enabled
        );
    }
    Ok(())
}

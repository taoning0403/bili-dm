use bili_dm_lib::{
    bilibili::discovery::{parse_episodes, pgc_id},
    danmaku::{
        alignment::{self, Anchor, Evidence, Segment},
        audio,
        service::requested_episode,
        strategy::MatchOptions,
    },
    player::audio_probe::AudioSamples,
};
use tokio_util::sync::CancellationToken;
fn segment(start: f64, end: f64, offset: f64, kind: Evidence) -> Segment {
    Segment {
        source_start: start,
        source_end: end,
        offset,
        score: 0.95,
        error: 0.1,
        kind,
        review: false,
    }
}
#[test]
fn pgc_identifiers_and_episode_metadata_are_distinct_from_bv_pages(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        pgc_id("https://www.bilibili.com/bangumi/play/ep42?x=1"),
        Some((true, 42))
    );
    assert_eq!(pgc_id("ss99"), Some((false, 99)));
    for bad in [
        "ep0",
        "ep-2",
        "https://bilibili.com.evil.test/bangumi/play/ep42",
        "https://user@bilibili.com/bangumi/play/ep42",
    ] {
        assert_eq!(pgc_id(bad), None);
    }
    let data = serde_json::json!({"title":"节目","episodes":[{"id":42,"cid":99,"title":"2","long_title":"标题","duration":120000,"bvid":"BV1xx411c7mD"},{"id":43,"cid":100,"title":"3","duration":125000}]});
    let parsed = parse_episodes(&data, Some(42))?;
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].duration, 120.0);
    assert_eq!(parsed[0].episode_id, Some(42));
    assert_eq!(parsed[0].page, 2);
    assert_eq!(requested_episode("Title S02E03"), Some(3));
    assert_eq!(requested_episode("作品 第12集"), Some(12));
    assert_eq!(requested_episode("作品 2024"), None);
    Ok(())
}
#[test]
fn strategy_rejects_invalid_numbers_and_can_include_short_versions() {
    let mut options = MatchOptions::default();
    assert!(!options.duration_allowed(95.0, 100.0));
    options.min_duration_ratio = 0.9;
    assert!(options.duration_allowed(95.0, 100.0));
    options.max_error = f64::NAN;
    assert!(options.validate().is_err());
}

#[test]
fn fixed_audio_delay_requires_consistent_long_visual_evidence() {
    let options = MatchOptions::default();
    let video = [segment(0.0, 40.0, 0.0, Evidence::Video)];
    let audio = [segment(0.0, 40.0, 1.2, Evidence::Audio)];
    assert_eq!(alignment::audio_delay(&video, &audio, &options), Some(-1.2));
    let conflicting = [
        segment(0.0, 20.0, 1.2, Evidence::Audio),
        segment(20.0, 40.0, -1.2, Evidence::Audio),
    ];
    assert_eq!(alignment::audio_delay(&video, &conflicting, &options), None);
    assert_eq!(
        alignment::audio_delay(&[segment(0.0, 5.0, 0.0, Evidence::Video)], &audio, &options),
        None
    );
}
#[test]
fn deletion_creates_offset_steps_without_stretching_missing_content() {
    let options = MatchOptions {
        allow_edits: true,
        ..Default::default()
    };
    let anchors = (0..100)
        .map(|i| {
            let s = i as f64;
            Anchor {
                source: s,
                target: s + if i < 40 { 0.0 } else { 12.0 },
                score: 1.0,
                kind: Evidence::Audio,
            }
        })
        .collect();
    let chain = alignment::chain(anchors, &options, 0.3);
    let segments = alignment::segments(&chain, 1.0, 0.3, 4.0, 100.0, 112.0);
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].offset, 0.0);
    assert_eq!(segments[1].offset, 12.0);
    assert!(
        (segments[1].source_start + segments[1].offset - segments[0].source_end - 12.0).abs()
            < 0.01
    );
}
#[test]
fn redraw_audio_is_accepted_only_when_bracketed_or_explicitly_enabled() {
    let options = MatchOptions::default();
    let video = [
        segment(0.0, 10.0, 2.0, Evidence::Video),
        segment(20.0, 30.0, 2.0, Evidence::Video),
    ];
    let audio = [segment(0.0, 30.0, 2.0, Evidence::Audio)];
    let combined = alignment::fuse(&video, &audio, &options);
    assert_eq!(combined.len(), 3);
    assert_eq!(combined[1].kind, Evidence::Audio);
    assert!(!combined[1].review);
    let only_audio = alignment::fuse(&[], &audio, &options);
    assert!(only_audio.iter().all(|s| s.review));
    let options = MatchOptions {
        allow_audio_only: true,
        ..options
    };
    assert!(!alignment::fuse(&[], &audio, &options)[0].review);
}
#[test]
fn conflicting_audio_video_and_reversed_target_ranges_need_review() {
    let options = MatchOptions::default();
    let video = [segment(0.0, 20.0, 0.0, Evidence::Video)];
    let audio = [segment(0.0, 20.0, 4.0, Evidence::Audio)];
    assert!(alignment::fuse(&video, &audio, &options)[0].review);
    let video = [
        segment(0.0, 10.0, 20.0, Evidence::Video),
        segment(10.0, 20.0, 0.0, Evidence::Video),
    ];
    assert!(alignment::fuse(&video, &[], &options)
        .iter()
        .all(|s| s.review));
}
fn signal(seconds: usize) -> AudioSamples {
    let rate = 11025;
    let samples = (0..seconds * rate)
        .map(|i| {
            let t = i as f64 / rate as f64;
            let step = (t * 3.0).floor();
            let a = 180.0 + (step * 37.0) % 1500.0;
            let b = 700.0 + (step * 73.0) % 2400.0;
            ((t * a * std::f64::consts::TAU).sin() * 0.25
                + (t * b * std::f64::consts::TAU).sin() * 0.2) as f32
        })
        .collect();
    AudioSamples {
        samples,
        sample_rate: rate as u32,
    }
}
#[test]
fn audio_fingerprints_recover_a_cropped_offset_and_reject_silence(
) -> Result<(), Box<dyn std::error::Error>> {
    let token = CancellationToken::new();
    let target = signal(36);
    let rate = target.sample_rate as usize;
    let source = AudioSamples {
        samples: target.samples[rate * 7..rate * 27]
            .iter()
            .map(|v| v * 0.45)
            .collect(),
        sample_rate: target.sample_rate,
    };
    let a = audio::fingerprint(&source, &token)?;
    let b = audio::fingerprint(&target, &token)?;
    let anchors = audio::anchors(&a, &b, &token)?;
    let correct = anchors
        .iter()
        .filter(|a| (a.offset() - 7.0).abs() < 0.15)
        .count();
    assert!(correct > 15, "correct={correct} anchors={}", anchors.len());
    let silent = AudioSamples {
        samples: vec![0.0; rate * 20],
        sample_rate: rate as u32,
    };
    assert!(audio::fingerprint(&silent, &token)?.marks.is_empty());
    Ok(())
}

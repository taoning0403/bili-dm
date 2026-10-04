use bili_dm_lib::{
    danmaku::{
        bilibili::{decode_segment, parse_input},
        matching,
        mixer::{mix, to_xml},
        models::*,
    },
    player::probe::{fingerprint, Frame},
};

#[test]
fn provider_uses_a_valid_backup_when_primary_is_an_unrecognized_cdn(
) -> Result<(), Box<dyn std::error::Error>> {
    use bili_dm_lib::danmaku::bilibili::select_stream_url;
    let video = serde_json::json!({"baseUrl":"https://edge.example.test/video", "backupUrl":["http://127.0.0.1/private", "https://upos-sz-mirrorcos.bilivideo.com/video?token=public"]});
    assert_eq!(
        select_stream_url(&video)?,
        "https://upos-sz-mirrorcos.bilivideo.com/video?token=public"
    );
    assert!(
        select_stream_url(&serde_json::json!({"url":"https://bilivideo.com.evil.test/video"}))
            .is_err()
    );
    Ok(())
}

fn source(id: &str) -> ParsedSource {
    ParsedSource {
        info: SourceInfo {
            id: id.into(),
            bvid: "BV1xx411c7mD".into(),
            cid: 1,
            page: 1,
            title: id.into(),
            duration: 100.0,
            comment_count: 4,
            warnings: vec![],
            episode_id: None,
        },
        comments: [0.0, 10.0, 20.0, 30.0]
            .iter()
            .enumerate()
            .map(|(i, t)| Comment {
                id: i.to_string(),
                time: *t,
                mode: 1,
                color: 0xffffff,
                size: 25,
                text: format!("弹幕 {i}"),
            })
            .collect(),
    }
}
fn clip(id: &str, ss: f64, se: f64, ts: f64, te: f64) -> Clip {
    Clip {
        id: "clip".into(),
        source_id: id.into(),
        source_start: ss,
        source_end: se,
        target_start: ts,
        target_end: te,
        enabled: true,
        confidence: None,
        evidence: None,
        review_required: false,
        evidence_kind: None,
    }
}

#[test]
fn parses_bv_links_and_explicit_parts_without_accepting_other_hosts(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(parse_input(" BV1xx411c7mD ")?.bvid, "BV1xx411c7mD");
    let parsed =
        parse_input("https://www.bilibili.com/video/BV1xx411c7mD/?p=2&spm_id_from=333.0#reply")?;
    assert_eq!(parsed.page, Some(2));
    for bad in [
        "BVwrong",
        "https://bilibili.com.evil.test/video/BV1xx411c7mD",
        "https://www.bilibili.com/video/BV1xx411c7mD?p=0",
        "file:///video/BV1xx411c7mD",
        "https://www.bilibili.com/bangumi/play/ep123",
        "https://user@www.bilibili.com/video/BV1xx411c7mD",
    ] {
        assert!(parse_input(bad).is_err(), "{bad}");
    }
    Ok(())
}
#[test]
fn protobuf_decode_preserves_millisecond_time_color_and_unicode(
) -> Result<(), Box<dyn std::error::Error>> {
    // Segment field 1, element: id=7 progress=1500 mode=5 fontsize=25 color=255 content=你好.
    let element = [
        8, 7, 16, 220, 11, 24, 5, 32, 25, 40, 255, 1, 58, 6, 228, 189, 160, 229, 165, 189,
    ];
    let mut bytes = vec![10, element.len() as u8];
    bytes.extend_from_slice(&element);
    let (comments, skipped) = decode_segment(&bytes)?;
    assert_eq!(skipped, 0);
    assert_eq!(comments[0].text, "你好");
    assert_eq!(comments[0].time, 1.5);
    assert_eq!(comments[0].mode, 5);
    assert_eq!(comments[0].color, 255);
    assert!(decode_segment(&bytes[..bytes.len() - 1]).is_err());
    assert!(decode_segment(b"<html>blocked</html>").is_err());
    assert!(decode_segment(&[])?.0.is_empty());
    Ok(())
}
#[test]
fn mixes_multiple_sources_scaling_clipping_and_preserving_distinct_posts(
) -> Result<(), Box<dyn std::error::Error>> {
    let track = mix(
        "local.mp4",
        60.0,
        &[source("a"), source("b")],
        &[
            clip("a", 10.0, 30.0, 0.0, 40.0),
            clip("b", 0.0, 20.0, 40.0, 60.0),
        ],
    )?;
    assert_eq!(
        track.comments.iter().map(|c| c.time).collect::<Vec<_>>(),
        [0.0, 20.0, 40.0, 50.0]
    );
    assert!(track.warnings.iter().any(|w| w.contains("2.0000")));
    let track = mix(
        "local.mp4",
        20.0,
        &[source("a"), source("b")],
        &[
            clip("a", 0.0, 20.0, 0.0, 20.0),
            clip("b", 0.0, 20.0, 0.0, 20.0),
        ],
    )?;
    assert_eq!(track.comments.len(), 4);
    Ok(())
}
#[test]
fn adjacent_cuts_and_repeated_mappings_do_not_duplicate_boundary_comments(
) -> Result<(), Box<dyn std::error::Error>> {
    let a = clip("a", 0.0, 20.0, 0.0, 20.0);
    let track = mix(
        "file",
        40.0,
        &[source("a")],
        &[a.clone(), a, clip("a", 20.0, 40.0, 20.0, 40.0)],
    )?;
    assert_eq!(track.comments.len(), 4);
    assert_eq!(track.comments.iter().filter(|c| c.time == 20.0).count(), 1);
    Ok(())
}
#[test]
fn rejects_invalid_ranges_and_reports_uncovered_time() -> Result<(), Box<dyn std::error::Error>> {
    for bad in [
        clip("a", 20.0, 10.0, 0.0, 10.0),
        clip("a", 0.0, 101.0, 0.0, 10.0),
        clip("a", 0.0, 10.0, 0.0, 41.0),
        clip("a", f64::NAN, 10.0, 0.0, 10.0),
        clip("missing", 0.0, 10.0, 0.0, 10.0),
    ] {
        assert!(mix("file", 40.0, &[source("a")], &[bad]).is_err());
    }
    let track = mix(
        "file",
        40.0,
        &[source("a")],
        &[clip("a", 0.0, 20.0, 10.0, 30.0)],
    )?;
    assert!(track.warnings.iter().any(|s| s.contains("20.0 秒")));
    Ok(())
}
#[test]
fn xml_exports_only_data_and_escapes_untrusted_text() -> Result<(), Box<dyn std::error::Error>> {
    let mut s = source("a");
    s.comments[0].text = "<b>&\"'\\N\u{0001}".into();
    let track = mix("file", 20.0, &[s], &[clip("a", 0.0, 20.0, 0.0, 20.0)])?;
    let xml = to_xml(&track);
    assert!(xml.contains("&lt;b&gt;&amp;&quot;&apos;\\N"));
    assert!(!xml.contains('\u{0001}'));
    Ok(())
}
fn frame(time: f64, seed: u32) -> Frame {
    let mut seed = seed + 1;
    let pixels: Vec<u8> = (0..160 * 90 * 3)
        .map(|i| {
            if i % 30 == 0 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            }
            (seed >> 24) as u8
        })
        .collect();
    fingerprint(time, 160, 90, &pixels)
}
#[test]
fn visual_matching_finds_offset_and_rejects_black_or_unrelated_footage(
) -> Result<(), Box<dyn std::error::Error>> {
    let source: Vec<_> = (0..20).map(|i| frame(i as f64 + 0.2, i)).collect();
    let target: Vec<_> = (0..30)
        .map(|i| {
            frame(
                i as f64 + 0.2,
                if (5..25).contains(&i) { i - 5 } else { i + 100 },
            )
        })
        .collect();
    assert_eq!(
        matching::candidates(&source, &target, 20.0, 30.0, 1.0).first(),
        Some(&5.0)
    );
    let anchors = vec![source[1].clone(), source[10].clone(), source[18].clone()];
    let matched =
        matching::refine(&anchors, &target, 5.0, 1.0, 20.0, 30.0).ok_or("no matching result")?;
    assert!((matched.target_start - 5.0).abs() < 0.01);
    assert!((matched.source_end - 20.0).abs() < 0.01);
    let black = fingerprint(0.0, 160, 90, &vec![0; 160 * 90 * 3]);
    assert_eq!(matching::distance(&black, &black), 1.0);
    let unrelated: Vec<_> = (0..30).map(|i| frame(i as f64 + 0.2, i + 500)).collect();
    assert!(matching::refine(&anchors, &unrelated, 5.0, 1.0, 20.0, 30.0).is_none());
    Ok(())
}
#[test]
fn matching_rejects_inconsistent_endpoints_despite_same_duration(
) -> Result<(), Box<dyn std::error::Error>> {
    let anchors = vec![frame(1.0, 1), frame(10.0, 2), frame(19.0, 3)];
    let target = vec![frame(6.0, 1), frame(15.0, 2), frame(26.0, 3)];
    assert!(matching::refine(&anchors, &target, 5.0, 3.0, 20.0, 30.0).is_none());
    Ok(())
}

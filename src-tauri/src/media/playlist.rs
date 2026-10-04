use super::files::{FileKind, MediaFile};
use std::{cmp::Ordering, path::Path};

/// Digit runs are compared without integer parsing, including very long names.
pub fn natural_order(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (mut x, mut y) = (a.as_bytes(), b.as_bytes());
    while !x.is_empty() && !y.is_empty() {
        if x[0].is_ascii_digit() && y[0].is_ascii_digit() {
            let nx = x.iter().take_while(|c| c.is_ascii_digit()).count();
            let ny = y.iter().take_while(|c| c.is_ascii_digit()).count();
            let dx = x[..nx].iter().position(|c| *c != b'0').unwrap_or(nx);
            let dy = y[..ny].iter().position(|c| *c != b'0').unwrap_or(ny);
            let order = (nx - dx)
                .cmp(&(ny - dy))
                .then_with(|| x[dx..nx].cmp(&y[dy..ny]));
            if !order.is_eq() {
                return order;
            }
            x = &x[nx..];
            y = &y[ny..];
        } else {
            let order = x[0].cmp(&y[0]);
            if !order.is_eq() {
                return order;
            }
            x = &x[1..];
            y = &y[1..];
        }
    }
    x.len().cmp(&y.len()).then_with(|| a.cmp(&b))
}
pub fn video_queue(files: &[MediaFile]) -> Vec<MediaFile> {
    let mut queue: Vec<_> = files
        .iter()
        .filter(|f| f.kind == FileKind::Video && f.size > 0)
        .cloned()
        .collect();
    queue.sort_by(|a, b| natural_order(&a.path, &b.path).then(a.index.cmp(&b.index)));
    queue
}
/// Exact stem or stem + language suffix, including a Subs/ subdirectory.
pub fn matching_subtitles(files: &[MediaFile], video: &MediaFile) -> Vec<MediaFile> {
    let stem = Path::new(&video.path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let single_video = video_queue(files).len() == 1;
    files
        .iter()
        .filter(|f| {
            if f.kind != FileKind::Subtitle || f.size == 0 || f.size > 20 * 1024 * 1024 {
                return false;
            }
            let candidate = Path::new(&f.path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            single_video
                || candidate == stem
                || candidate
                    .strip_prefix(&stem)
                    .is_some_and(|suffix| suffix.starts_with(['.', '_', '-', ' ']))
        })
        .cloned()
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn natural_queue_preserves_engine_indices() {
        let files = vec![
            MediaFile::new(4, "E10.mkv".into(), 1),
            MediaFile::new(8, "E2.mkv".into(), 1),
            MediaFile::new(2, "E01.mkv".into(), 1),
            MediaFile::new(9, "empty.mp4".into(), 0),
        ];
        assert_eq!(
            video_queue(&files)
                .iter()
                .map(|f| f.index)
                .collect::<Vec<_>>(),
            vec![2, 8, 4]
        );
        assert!(natural_order("99999999999999999999", "100000000000000000000").is_lt());
    }
    #[test]
    fn subtitles_do_not_cross_episode_boundaries() {
        let files = vec![
            MediaFile::new(0, "E01.mkv".into(), 1),
            MediaFile::new(1, "E02.mkv".into(), 1),
            MediaFile::new(2, "Subs/E01.zh.ass".into(), 12),
            MediaFile::new(3, "E010.srt".into(), 12),
            MediaFile::new(4, "E02.srt".into(), 12),
        ];
        assert_eq!(
            matching_subtitles(&files, &files[0])
                .iter()
                .map(|f| f.index)
                .collect::<Vec<_>>(),
            vec![2]
        );
    }
}

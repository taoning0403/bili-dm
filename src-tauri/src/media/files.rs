use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Video,
    Subtitle,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub index: usize,
    pub path: String,
    pub size: u64,
    pub kind: FileKind,
}

pub fn classify(path: &str) -> FileKind {
    match Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "mkv" | "avi" | "webm" | "mov" | "m4v" | "ts" | "m2ts" => FileKind::Video,
        "srt" | "ass" | "ssa" | "vtt" | "sub" | "idx" => FileKind::Subtitle,
        _ => FileKind::Other,
    }
}

impl MediaFile {
    pub fn new(index: usize, path: String, size: u64) -> Self {
        Self {
            index,
            kind: classify(&path),
            path,
            size,
        }
    }
}

pub fn preferred_video(files: &[MediaFile]) -> Option<usize> {
    // Keep the first file on equal sizes for a stable default.
    files
        .iter()
        .filter(|file| file.kind == FileKind::Video && file.size > 0)
        .fold(None::<&MediaFile>, |largest, file| {
            if largest.is_none_or(|current| file.size > current.size) {
                Some(file)
            } else {
                largest
            }
        })
        .map(|file| file.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_videos_subtitles_and_misleading_extensions() {
        assert_eq!(classify("目录/Episode.MKV"), FileKind::Video);
        assert_eq!(classify("subtitles/中文.ASS"), FileKind::Subtitle);
        assert_eq!(classify("movie.mp4.exe"), FileKind::Other);
        assert_eq!(classify("cover.jpg"), FileKind::Other);
    }

    #[test]
    fn prefers_largest_nonempty_video_and_preserves_original_index() {
        let files = [
            MediaFile::new(7, "sample.mp4".into(), 10),
            MediaFile::new(12, "movie.mkv".into(), 100),
            MediaFile::new(99, "poster.jpg".into(), 200),
            MediaFile::new(15, "tie.mov".into(), 100),
        ];
        assert_eq!(preferred_video(&files), Some(12));
        assert_eq!(
            preferred_video(&[MediaFile::new(0, "empty.mp4".into(), 0)]),
            None
        );
        assert_eq!(preferred_video(&[]), None);
    }
}

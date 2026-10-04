import type { TorrentFile } from "./torrent";
export interface ActiveMedia { sessionId: string; title: string; source: "local" | "torrent"; torrentId: string | null; fileIndex: number | null; localPath: string | null; resumedFrom: number }
export interface MediaTrack { id: number; kind: "video" | "audio" | "sub"; title: string; language: string; codec: string; selected: boolean; external: boolean }
export interface PlayerSnapshot {
  sampledAtMs: number; videoAspect: number;
  running: boolean; loaded: boolean; paused: boolean; buffering: boolean; ended: boolean;
  position: number; duration: number; volume: number; muted: boolean; speed: number;
  subtitleDelay: number; audioDelay: number; cacheSeconds: number; decoder: string;
  tracks: MediaTrack[]; chapters: { title: string; time: number }[]; error: string | null;
}
export interface DownloadStats {
  torrentId: string; fileIndex: number; downloaded: number; total: number; bytesPerSecond: number; peers: number; state: string; error: string | null;
  buffered: { start: number; end: number }[]; selectedFiles: number[];
  prefetch: { fileIndex: number; downloaded: number; total: number } | null;
}
export interface QueueOptions { autoNext: boolean; prefetchNext: boolean; repeat: "off" | "one" | "all" }
export interface PlaybackPreferences extends QueueOptions { volume: number; muted: boolean; speed: number }
export interface PlaybackState {
  fullscreen: boolean;
  media: ActiveMedia | null; player: PlayerSnapshot; download: DownloadStats | null;
  queue: TorrentFile[]; queueIndex: number | null; preferences: PlaybackPreferences; warning: string | null;
}
export interface VideoViewport { left: number; right: number; top: number; bottom: number }
export type PlayerControl =
  | { type: "pause"; paused: boolean } | { type: "seek"; seconds: number }
  | { type: "volume"; volume: number } | { type: "mute"; muted: boolean }
  | { type: "speed"; speed: number } | { type: "audioTrack" | "subtitleTrack"; id: number }
  | { type: "subtitleDelay" | "audioDelay"; seconds: number } | { type: "frameStep"; backwards: boolean }
  | { type: "stop" };

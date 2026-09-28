export interface ActiveMedia {
  title: string;
  source: "local" | "torrent";
  torrentId: string | null;
  fileIndex: number | null;
  localPath: string | null;
}

export interface PlayerSnapshot {
  running: boolean;
  loaded: boolean;
  paused: boolean;
  buffering: boolean;
  ended: boolean;
  position: number;
  duration: number;
  volume: number;
  error: string | null;
}

export interface DownloadStats {
  torrentId: string;
  fileIndex: number;
  downloaded: number;
  total: number;
  bytesPerSecond: number;
  peers: number;
  state: string;
  error: string | null;
}

export interface PlaybackState {
  media: ActiveMedia | null;
  player: PlayerSnapshot;
  download: DownloadStats | null;
}

export type PlayerControl =
  | { type: "pause"; paused: boolean }
  | { type: "seek"; seconds: number }
  | { type: "volume"; volume: number }
  | { type: "stop" };

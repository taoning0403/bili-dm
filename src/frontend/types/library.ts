export interface TorrentRecord {
  torrentId: string;
  magnetUri: string;
  name: string;
  createdAt: number;
  status: "ready" | "downloading" | "paused";
  fileCount: number;
}

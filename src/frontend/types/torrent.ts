export interface TorrentFile {
  index: number;
  path: string;
  size: number;
  kind: "video" | "subtitle" | "other";
}

export interface TorrentCatalog {
  id: string;
  name: string;
  files: TorrentFile[];
  suggestedFileIndex: number | null;
}

export interface TorrentFile {
  index: number;
  path: string;
  size: number;
}

export interface TorrentCatalog {
  id: string;
  name: string;
  files: TorrentFile[];
}

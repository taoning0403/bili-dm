import { invokeDesktop } from "../lib/desktop";
import type { TorrentRecord } from "../types/library";

export const libraryService = {
  recentTorrents: () => invokeDesktop<TorrentRecord[]>("recent_torrents"),
};

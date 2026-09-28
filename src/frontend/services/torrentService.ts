import { invokeDesktop } from "../lib/desktop";
import type { TorrentCatalog } from "../types/torrent";

export const torrentService = {
  resolve: (magnet: string) => invokeDesktop<TorrentCatalog>("resolve_magnet", { magnet }, 130_000),
  cancel: () => invokeDesktop<void>("cancel_magnet"),
};

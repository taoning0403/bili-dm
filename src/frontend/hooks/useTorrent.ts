import { useState } from "react";
import { torrentService } from "../services/torrentService";
import { getErrorMessage } from "../lib/desktop";
import type { TorrentCatalog } from "../types/torrent";

export function useTorrent() {
  const [catalog, setCatalog] = useState<TorrentCatalog | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [selectedFile, setSelectedFile] = useState<number | null>(null);

  async function load(magnet: string) {
    setLoading(true);
    setError("");
    try {
      const resolved = await torrentService.resolve(magnet);
      setCatalog(resolved);
      setSelectedFile(resolved.suggestedFileIndex);
    } catch (cause) {
      setError(getErrorMessage(cause));
    } finally {
      setLoading(false);
    }
  }

  async function cancel() {
    try { await torrentService.cancel(); }
    catch (cause) { setError(getErrorMessage(cause)); }
  }

  return { catalog, loading, error, load, cancel, selectedFile, selectFile: setSelectedFile };
}

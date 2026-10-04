import { useRef, useState } from "react";
import { torrentService } from "../services/torrentService";
import { invokeDesktop, getErrorMessage } from "../lib/desktop";
import type { TorrentCatalog } from "../types/torrent";
export function useTorrent() {
  const [catalog, setCatalog] = useState<TorrentCatalog | null>(null);
  const [loading, setLoading] = useState(false), [error, setError] = useState("");
  const [selectedFile, setSelectedFile] = useState<number | null>(null);
  const pending = useRef(false);
  async function resolve(action: () => Promise<TorrentCatalog | null>) {
    if (pending.current) return;
    pending.current = true; setLoading(true); setError("");
    try {
      const resolved = await action();
      if (resolved) { setCatalog(resolved); setSelectedFile(resolved.suggestedFileIndex); }
    } catch (cause) { setError(getErrorMessage(cause)); }
    finally { pending.current = false; setLoading(false); }
  }
  async function cancel() { try { await torrentService.cancel(); } catch (cause) { setError(getErrorMessage(cause)); } }
  return {
    catalog, loading, error, cancel, selectedFile, selectFile: setSelectedFile,
    load: (magnet: string) => resolve(() => torrentService.resolve(magnet)),
    openFile: () => resolve(() => invokeDesktop<TorrentCatalog | null>("open_torrent_file", undefined, 0)),
  };
}

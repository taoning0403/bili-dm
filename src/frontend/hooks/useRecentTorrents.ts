import { useEffect, useState } from "react";
import { libraryService } from "../services/libraryService";
import { getErrorMessage } from "../lib/desktop";
import type { TorrentRecord } from "../types/library";
import type { TorrentCatalog } from "../types/torrent";

export function useRecentTorrents(catalog: TorrentCatalog | null, activeId: string | null | undefined) {
  const [tasks, setTasks] = useState<TorrentRecord[]>([]);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    void libraryService.recentTorrents().then((next) => {
      if (active) { setTasks(next); setError(""); }
    }).catch((cause: unknown) => { if (active) setError(getErrorMessage(cause)); });
    return () => { active = false; };
  }, [catalog, activeId]);
  return { tasks, error };
}

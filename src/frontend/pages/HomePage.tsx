import { AppHeader } from "../components/AppHeader";
import { MagnetInput } from "../components/MagnetInput";
import { useTorrent } from "../hooks/useTorrent";
import { FileCatalog } from "../components/FileCatalog";
import { PlayerControls } from "../components/PlayerControls";
import { usePlayback } from "../hooks/usePlayback";
import { useState } from "react";
import { useRecentTorrents } from "../hooks/useRecentTorrents";
import { RecentTorrents } from "../components/RecentTorrents";

export function HomePage() {
  const torrent = useTorrent();
  const playback = usePlayback();
  const recent = useRecentTorrents(torrent.catalog, playback.state?.media?.torrentId);
  const [magnet, setMagnet] = useState("");
  async function load(value: string) {
    setMagnet(value);
    await torrent.load(value);
  }

  return (
    <div className="app-shell">
      <AppHeader onOpenLocal={playback.openLocal} busy={playback.busy} />
      <main>
        <PlayerControls key={`${playback.state?.media?.source}:${playback.state?.media?.localPath ?? playback.state?.media?.torrentId}:${playback.state?.media?.fileIndex}`} state={playback.state} busy={playback.busy} onControl={playback.control} />
        {playback.error && <p className="error-message" role="alert">{playback.error}</p>}
        <MagnetInput magnet={magnet} onChange={setMagnet} loading={torrent.loading} onLoad={load} onCancel={torrent.cancel} />
        {recent.error && <p className="error-message" role="alert">历史记录：{recent.error}</p>}
        <RecentTorrents tasks={recent.tasks} disabled={torrent.loading} onLoad={load} />
        {torrent.loading && <p role="status">正在从节点读取元数据…</p>}
        {torrent.error && <p className="error-message" role="alert">{torrent.error}</p>}
        {torrent.catalog && <FileCatalog catalog={torrent.catalog} selected={torrent.selectedFile} onSelect={torrent.selectFile} />}
        {torrent.catalog && <div className="catalog-actions">
          <p className="muted">{torrent.selectedFile === null ? "请选择目录中的视频" : torrent.selectedFile === torrent.catalog.suggestedFileIndex ? "已选中最大视频，可手动改选" : "已按你的选择切换视频"} · 字幕目前仅识别</p>
          <button className="primary-button" disabled={torrent.selectedFile === null || playback.busy || torrent.loading}
            onClick={() => { if (torrent.catalog && torrent.selectedFile !== null) void playback.playTorrent(torrent.catalog.id, torrent.selectedFile); }}>
            {playback.busy ? "正在打开…" : "播放选中视频"}
          </button>
        </div>}
      </main>
      <footer className="app-footer">无需账号 · 本地优先 · torrent 数据保存在本机缓存目录</footer>
    </div>
  );
}

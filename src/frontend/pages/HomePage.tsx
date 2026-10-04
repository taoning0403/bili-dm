import { useEffect, useState } from "react";
import { MagnetInput } from "../components/MagnetInput";
import { useTorrent } from "../hooks/useTorrent";
import { FileCatalog } from "../components/FileCatalog";
import { PlayerControls } from "../components/PlayerControls";
import { VideoSurface } from "../components/VideoSurface";
import { PlaybackQueue } from "../components/PlaybackQueue";
import { usePlayback } from "../hooks/usePlayback";
import { useRecentTorrents } from "../hooks/useRecentTorrents";
import { RecentTorrents } from "../components/RecentTorrents";

export function HomePage() {
  const torrent = useTorrent(), playback = usePlayback();
  const recent = useRecentTorrents(torrent.catalog, playback.state?.media?.torrentId);
  const [magnet, setMagnet] = useState("");
  const [tab, setTab] = useState<"source" | "queue">("source");
  async function load(value: string) { setMagnet(value); await torrent.load(value); }

  useEffect(() => {
    function key(event: KeyboardEvent) {
      if (event.target instanceof HTMLElement && (event.target.closest("input,textarea,select") || event.target.isContentEditable
        || (event.target.closest("button") && ["Space", "Enter"].includes(event.code)))) return;
      const p = playback.state?.player;
      if (event.code === "KeyF") { event.preventDefault(); void playback.toggleFullscreen(); return; }
      if (event.code === "Escape" && playback.fullscreen) { event.preventDefault(); void playback.toggleFullscreen(false); return; }
      if (!p?.loaded || playback.busy) return;
      const controls = playback.control;
      switch (event.code) {
        case "Space": case "KeyK": void controls({ type: "pause", paused: !p.paused }); break;
        case "ArrowLeft": void controls({ type: "seek", seconds: Math.max(0, p.position - (event.shiftKey ? 1 : 10)) }); break;
        case "ArrowRight": void controls({ type: "seek", seconds: Math.min(p.duration, p.position + (event.shiftKey ? 1 : 10)) }); break;
        case "ArrowUp": void controls({ type: "volume", volume: Math.min(100, p.volume + 5) }); break;
        case "ArrowDown": void controls({ type: "volume", volume: Math.max(0, p.volume - 5) }); break;
        case "KeyM": void controls({ type: "mute", muted: !p.muted }); break;
        case "Comma": void controls({ type: "frameStep", backwards: true }); break;
        case "Period": void controls({ type: "frameStep", backwards: false }); break;
        case "PageUp": if ((playback.state?.queueIndex ?? 0) > 0) void playback.selectQueue((playback.state?.queueIndex ?? 0) - 1); break;
        case "PageDown": if (playback.state?.queueIndex != null && playback.state.queueIndex + 1 < playback.state.queue.length) void playback.selectQueue(playback.state.queueIndex + 1); break;
        default: return;
      }
      event.preventDefault();
    }
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [playback]);

  const state = playback.state;
  return <div className={"app-shell" + (navigator.platform.startsWith("Mac") ? " macos" : "") + (playback.fullscreen ? " is-fullscreen" : "")}>
    <header className="app-header">
      <div className="brand"><span className="brand-mark" aria-hidden="true">▶</span><div><strong>Bili DM</strong><span>本地影院</span></div></div>
      <div className="header-actions">
        <button onClick={() => void torrent.openFile()} disabled={torrent.loading}>打开种子</button>
        <button className="primary-button" onClick={() => void playback.openLocal()} disabled={playback.busy}>打开本地视频</button>
      </div>
    </header>
    <main className="workspace">
      <div className="screen-column">
        <VideoSurface state={state} busy={playback.busy} onToggle={() => void playback.control({ type: "pause", paused: !state?.player.paused })} onFullscreen={() => void playback.toggleFullscreen()} />
        <PlayerControls key={state?.media?.localPath ?? (state?.media?.torrentId ?? "") + ":" + state?.media?.fileIndex}
          state={state} busy={playback.busy} fullscreen={playback.fullscreen} onControl={playback.control}
          onQueue={playback.selectQueue} onSubtitle={playback.openSubtitle} onFullscreen={() => void playback.toggleFullscreen()} />
        {(playback.error || state?.warning) && <div className="playback-alert" role="alert">{playback.error || state?.warning}</div>}
        <div className="player-footer"><span>LIBMPV <i /> 原生播放</span><span>Space 暂停 · ← → 跳转 · F 全屏</span></div>
      </div>
      <aside className="library-sidebar">
        <div className="sidebar-tabs" role="tablist" aria-label="资源与队列">
          <button role="tab" aria-selected={tab === "source"} aria-controls="source-panel" id="source-tab" onClick={() => setTab("source")}>打开资源</button>
          <button role="tab" aria-selected={tab === "queue"} aria-controls="queue-panel" id="queue-tab" onClick={() => setTab("queue")}>播放队列 <span>{state?.queue.length || ""}</span></button>
        </div>
        <div className="sidebar-content">
          {tab === "source" ? <div role="tabpanel" id="source-panel" aria-labelledby="source-tab">
            <MagnetInput magnet={magnet} onChange={setMagnet} loading={torrent.loading} onLoad={load} onCancel={torrent.cancel} />
            {torrent.loading && <p className="loading-message" role="status">正在读取资源目录…</p>}
            {torrent.error && <p className="error-message" role="alert">{torrent.error}</p>}
            {torrent.catalog && <><FileCatalog catalog={torrent.catalog} selected={torrent.selectedFile} onSelect={torrent.selectFile} />
              <button className="primary-button play-selected" disabled={torrent.selectedFile === null || playback.busy || torrent.loading}
                onClick={() => { if (torrent.catalog && torrent.selectedFile !== null) { void playback.playTorrent(torrent.catalog.id, torrent.selectedFile); setTab("queue"); } }}>播放选中视频</button>
            </>}
            <RecentTorrents tasks={recent.tasks} disabled={torrent.loading} onLoad={load} />
            {recent.error && <p className="error-message">{recent.error}</p>}
          </div> : <div role="tabpanel" id="queue-panel" aria-labelledby="queue-tab"><PlaybackQueue state={state} busy={playback.busy} onSelect={playback.selectQueue} onOptions={playback.options} /></div>}
        </div>
        <div className="sidebar-footer">无需账号 · 缓存与记录保存在本机</div>
      </aside>
    </main>
  </div>;
}

import { useState } from "react";
import type { PlaybackState, PlayerControl } from "../types/player";
import { formatBytes, formatTime } from "../lib/format";

interface Props { state: PlaybackState | null; busy: boolean; onControl: (control: PlayerControl) => Promise<void> }

export function PlayerControls({ state, busy, onControl }: Props) {
  const [seek, setSeek] = useState<number | null>(null);
  const [volume, setVolume] = useState<number | null>(null);
  const player = state?.player;
  const download = state?.download;
  const position = seek ?? player?.position ?? 0;
  const duration = player?.duration ?? 0;
  const loaded = player?.loaded ?? false;
  const status = busy ? "正在处理…" : !state?.media ? "尚未播放" : player?.error ? "播放失败"
    : player?.ended ? "播放结束" : !loaded || player?.buffering ? "正在缓冲…" : player?.paused ? "已暂停" : "正在播放";

  function commitSeek() {
    if (seek !== null) { void onControl({ type: "seek", seconds: seek }); setSeek(null); }
  }
  function commitVolume() {
    if (volume !== null) { void onControl({ type: "volume", volume }); setVolume(null); }
  }

  return <section className="player-panel" aria-label="播放控制">
    <div className="now-playing">
      <span className="player-glyph" aria-hidden="true">▶</span>
      <div className="playing-copy">
        <p className="eyebrow">{state?.media?.source === "torrent" ? "TORRENT" : "LOCAL PLAYER"}</p>
        <h2>{state?.media?.title ?? "选择视频，开始播放"}</h2>
        <p className="muted">视频在独立 mpv 窗口播放，可在这里控制进度和音量。</p>
      </div>
      <span className="playback-status" role="status">{status}</span>
    </div>
    {player?.error && <p className="error-message" role="alert">{player.error}</p>}
    <div className="timeline">
      <span>{formatTime(position)}</span>
      <input type="range" aria-label="播放进度" min={0} max={duration || 1} step={0.1}
        value={Math.min(position, duration || 1)} disabled={!loaded || busy}
        onChange={(event) => setSeek(Number(event.target.value))} onPointerUp={commitSeek} onKeyUp={commitSeek} onBlur={commitSeek} />
      <span>{formatTime(duration)}</span>
    </div>
    <div className="player-buttons">
      <button className="primary-button" disabled={!loaded || busy} onClick={() => void onControl({ type: "pause", paused: !player?.paused })}>
        {player?.paused ? "继续播放" : "暂停"}
      </button>
      <button disabled={!loaded || busy} onClick={() => void onControl({ type: "seek", seconds: Math.max(0, position - 10) })}>后退 10 秒</button>
      <button disabled={!loaded || busy} onClick={() => void onControl({ type: "seek", seconds: Math.min(duration, position + 10) })}>前进 10 秒</button>
      <button disabled={!state?.media || busy} onClick={() => void onControl({ type: "stop" })}>停止</button>
      <label className="volume-control">音量
        <input type="range" aria-label="音量" min={0} max={100} step={1} value={volume ?? player?.volume ?? 100}
          disabled={!player?.running || busy} onChange={(event) => setVolume(Number(event.target.value))}
          onPointerUp={commitVolume} onKeyUp={commitVolume} onBlur={commitVolume} />
        <span>{Math.round(volume ?? player?.volume ?? 100)}%</span>
      </label>
    </div>
    {download && <div className="download-status">
      <div><span>下载 {formatBytes(download.downloaded)} / {formatBytes(download.total)}</span>
        <span>{formatBytes(download.bytesPerSecond)}/s · {download.peers} 个连接</span></div>
      <progress aria-label="视频下载进度" value={download.downloaded} max={download.total || 1} />
      {download.error && <p role="alert" className="error-message">{download.error}</p>}
    </div>}
  </section>;
}

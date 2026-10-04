import { useEffect, useId, useRef, useState } from "react";
import type { PlaybackState, PlayerControl, MediaTrack } from "../types/player";
import { formatBytes, formatTime } from "../lib/format";
import { PlayerIcon } from "./PlayerIcon";
interface Props {
  state: PlaybackState | null; busy: boolean; fullscreen: boolean;
  onControl: (control: PlayerControl) => Promise<void>; onQueue: (index: number) => Promise<void>;
  onSubtitle: () => Promise<void>; onFullscreen: () => void;
  danmakuVisible: boolean; hasDanmaku: boolean; onToggleDanmaku: () => void; onConfigureDanmaku: () => void;
}
function trackName(track: MediaTrack) { return [track.title, track.language, track.codec].filter(Boolean).join(" · ") || "轨道 " + track.id; }
export function PlayerControls({ state, busy, fullscreen, onControl, onQueue, onSubtitle, onFullscreen, danmakuVisible, hasDanmaku, onToggleDanmaku, onConfigureDanmaku }: Props) {
  const [seek, setSeek] = useState<number | null>(null), [volume, setVolume] = useState<number | null>(null);
  const seekRef = useRef<number | null>(null), volumeRef = useRef<number | null>(null);
  const [settings, setSettings] = useState(false);
  const settingsRoot = useRef<HTMLDivElement>(null), settingsButton = useRef<HTMLButtonElement>(null);
  const settingsId = useId();
  useEffect(() => {
    if (!settings) return;
    function outside(event: PointerEvent) {
      if (event.target instanceof Node && !settingsRoot.current?.contains(event.target)) setSettings(false);
    }
    function escape(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault(); event.stopPropagation(); setSettings(false); settingsButton.current?.focus();
    }
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("pointerdown", outside); document.removeEventListener("keydown", escape); };
  }, [settings]);
  const player = state?.player, download = state?.download;
  const position = seek ?? player?.position ?? 0, duration = player?.duration ?? 0;
  const loaded = player?.loaded ?? false, queueIndex = state?.queueIndex;
  const status = busy ? "正在打开" : !state?.media ? "准备就绪" : player?.error ? "播放失败"
    : player?.ended ? "播放结束" : !loaded || player?.buffering ? "正在缓冲" : player?.paused ? "已暂停" : "正在播放";
  function commitSeek() {
    const value = seekRef.current; seekRef.current = null;
    if (value !== null) void onControl({ type: "seek", seconds: value }).finally(() => setSeek(null));
  }
  function commitVolume() {
    const value = volumeRef.current; volumeRef.current = null;
    if (value !== null) void onControl({ type: "volume", volume: value }).finally(() => setVolume(null));
  }
  const trackSelect = (kind: "audio" | "sub") => <select aria-label={kind === "audio" ? "音轨" : "字幕轨道"} disabled={!loaded || busy}
    value={player?.tracks.find(t => t.kind === kind && t.selected)?.id ?? 0}
    onChange={e => void onControl({ type: kind === "audio" ? "audioTrack" : "subtitleTrack", id: Number(e.target.value) })}>
    <option value={0}>{kind === "audio" ? "关闭音轨" : "关闭字幕"}</option>
    {player?.tracks.filter(t => t.kind === kind).map(t => <option key={t.id} value={t.id}>{trackName(t)}</option>)}
  </select>;
  return <section className="player-controls" aria-label="播放控制">
    <div className="timeline">
      <div className="timeline-rail">
        <div className="buffer-map" title="已校验的缓存分片，按文件字节比例估算时间位置">
          {download?.buffered.map((range, i) => <i key={i} style={{ left: range.start * 100 + "%", width: (range.end - range.start) * 100 + "%" }} />)}
          <b style={{ width: (duration > 0 ? Math.min(100, position / duration * 100) : 0) + "%" }} />
        </div>
        <input type="range" aria-label="播放进度" min={0} max={duration || 1} step={0.1} value={Math.min(position, duration || 1)} disabled={!loaded || busy}
          onChange={e => { const value = Number(e.target.value); seekRef.current = value; setSeek(value); }}
          onPointerUp={commitSeek} onKeyUp={commitSeek} onBlur={commitSeek} />
      </div>
    </div>
    <div className="player-buttons">
      <button className="icon-button play-button" title="播放 / 暂停 · Space" disabled={!loaded || busy} aria-label={!loaded ? "播放" : player?.paused ? "继续播放" : "暂停"} onClick={() => void onControl({ type: "pause", paused: !player?.paused })}><PlayerIcon name={!loaded || player?.paused ? "play" : "pause"} /></button>
      <button className="icon-button" title="下一集 · PageDown" aria-label="下一集" disabled={busy || queueIndex == null || queueIndex >= (state?.queue.length ?? 0) - 1} onClick={() => { if (queueIndex != null) void onQueue(queueIndex + 1); }}><PlayerIcon name="next" /></button>
      <span className="playback-time" title={status + (loaded ? " · 缓冲 " + Math.round(player?.cacheSeconds ?? 0) + " 秒" : "")}><span>{formatTime(position)}</span><span className="time-divider">/</span>{formatTime(duration)}</span>
      <div className="danmaku-controls">
        <button className="danmaku-toggle" title={hasDanmaku ? "显示 / 隐藏弹幕" : "请先在混合弹幕中添加弹幕源"} aria-label="显示弹幕" aria-pressed={danmakuVisible} disabled={!hasDanmaku} onClick={onToggleDanmaku}><span>弹</span><i aria-hidden="true" /></button>
        <button className="icon-button" title="配置混合弹幕" aria-label="配置混合弹幕" onClick={onConfigureDanmaku}><PlayerIcon name="danmakuSettings" /></button>
      </div>
      <div className="control-spacer" />
      <select aria-label="播放速度" title="播放速度" className="speed-select" disabled={!loaded || busy} value={player?.speed || 1} onChange={e => void onControl({ type: "speed", speed: Number(e.target.value) })}>
        {[0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4].map(speed => <option key={speed} value={speed}>{speed === 1 ? "倍速" : speed + "×"}</option>)}
      </select>
      <div className="volume-control">
        <button className="icon-button" title={player?.muted ? "取消静音 · M" : "音量 / 静音 · M"} aria-label={player?.muted ? "取消静音" : "静音"} disabled={!loaded || busy} onClick={() => void onControl({ type: "mute", muted: !player?.muted })}><PlayerIcon name={player?.muted ? "muted" : "volume"} /></button>
        <div className="volume-popover">
          <input className="volume-slider" type="range" aria-label="音量" min={0} max={100} value={volume ?? (loaded ? player?.volume : state?.preferences.volume) ?? 100} disabled={!loaded || busy}
            onChange={e => { const value = Number(e.target.value); volumeRef.current = value; setVolume(value); }}
            onPointerUp={commitVolume} onKeyUp={commitVolume} onBlur={commitVolume} />
          <span>{Math.round(volume ?? player?.volume ?? state?.preferences.volume ?? 100)}</span>
        </div>
      </div>
      <div className="settings-control" ref={settingsRoot}>
        <button className="icon-button" ref={settingsButton} title="播放设置 · 音轨 / 字幕" aria-label="播放设置" aria-expanded={settings} aria-controls={settingsId} onClick={() => setSettings(!settings)}><PlayerIcon name="settings" /></button>
        {settings && <div className="track-settings" id={settingsId} role="dialog" aria-label="播放设置">
          <div className="settings-heading"><h2>播放设置</h2><button className="icon-button" aria-label="关闭播放设置" onClick={() => { setSettings(false); settingsButton.current?.focus(); }}><PlayerIcon name="close" /></button></div>
          <div className="track-options"><label>音轨{trackSelect("audio")}</label><label>字幕{trackSelect("sub")}</label></div>
          <button className="load-subtitle" disabled={!loaded || busy} onClick={() => void onSubtitle()}>加载外挂字幕</button>
          <div className="delay-options">
            <label>字幕延迟 <select aria-label="字幕延迟" disabled={!loaded || busy} value={player?.subtitleDelay ?? 0} onChange={e => void onControl({ type: "subtitleDelay", seconds: Number(e.target.value) })}>
              {[-10,-5,-2,-1,-0.5,0,0.5,1,2,5,10].map(v => <option key={v} value={v}>{v > 0 ? "+" : ""}{v} 秒</option>)}
            </select></label>
            <label>音频延迟 <select aria-label="音频延迟" disabled={!loaded || busy} value={player?.audioDelay ?? 0} onChange={e => void onControl({ type: "audioDelay", seconds: Number(e.target.value) })}>
              {[-2,-1,-0.5,0,0.5,1,2].map(v => <option key={v} value={v}>{v > 0 ? "+" : ""}{v} 秒</option>)}
            </select></label>
          </div>
          <div className="transport-options">
            <button title="上一集 · PageUp" disabled={busy || queueIndex == null || queueIndex <= 0} onClick={() => { if (queueIndex != null) void onQueue(queueIndex - 1); }}>上一集</button>
            <button title="后退 10 秒 · ←" disabled={!loaded || busy} onClick={() => void onControl({ type: "seek", seconds: Math.max(0, position - 10) })}>−10 秒</button>
            <button title="前进 10 秒 · →" disabled={!loaded || busy} onClick={() => void onControl({ type: "seek", seconds: Math.min(duration, position + 10) })}>+10 秒</button>
            <button disabled={!state?.media && !busy} onClick={() => void onControl({ type: "stop" })}>{busy ? "取消打开" : "停止"}</button>
          </div>
          {!!player?.chapters.length && <div className="chapter-strip">{player.chapters.map((chapter, index) => <button key={index} disabled={busy} title={formatTime(chapter.time)}
            onClick={() => void onControl({ type: "seek", seconds: chapter.time })}>{chapter.title}</button>)}</div>}
          <p className="settings-status">{status}{loaded ? " · 缓冲 " + Math.round(player?.cacheSeconds ?? 0) + " 秒" : ""}<br />解码：{player?.decoder && player.decoder !== "no" ? player.decoder : "软件解码"}</p>
          <p className="settings-shortcuts">Space 暂停 · ← → 跳转 · F 全屏 · , / . 逐帧</p>
          {state?.media && state.media.resumedFrom > 0 && <p className="resume-note">已从 {formatTime(state.media.resumedFrom)} 续播</p>}
        </div>}
      </div>
      <button className="icon-button" title={fullscreen ? "退出全屏 · F" : "全屏 · F"} aria-label={fullscreen ? "退出全屏" : "全屏"} onClick={onFullscreen}><PlayerIcon name={fullscreen ? "exitFullscreen" : "fullscreen"} /></button>
    </div>
    {download && <div className="download-status">
      <div><span className="download-dot" />{formatBytes(download.bytesPerSecond)}/s <em>·</em> {download.peers} 个连接 <span className="control-spacer" />
        <span>{formatBytes(download.downloaded)} / {formatBytes(download.total)} · {download.total ? (100 * download.downloaded / download.total).toFixed(1) : "0"}%</span></div>
      {download.prefetch && <p>下一集已缓存 {formatBytes(download.prefetch.downloaded)} / {formatBytes(download.prefetch.total)}</p>}
      {download.error && <p role="alert" className="error-message">{download.error}</p>}
    </div>}
  </section>;
}

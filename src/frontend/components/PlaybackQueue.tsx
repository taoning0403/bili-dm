import { formatBytes } from "../lib/format";
import type { PlaybackState, QueueOptions } from "../types/player";
interface Props { state: PlaybackState | null; busy: boolean; onSelect: (index: number) => Promise<void>; onOptions: (options: QueueOptions) => Promise<void> }
export function PlaybackQueue({ state, busy, onSelect, onOptions }: Props) {
  const preferences = state?.preferences ?? { autoNext: true, prefetchNext: true, repeat: "off" as const };
  return <section className="queue-panel" aria-label="播放队列">
    <div className="section-heading"><h2>播放队列</h2><span>{state?.queue.length ?? 0} 集</span></div>
    <div className="queue-options">
      <label><input type="checkbox" checked={preferences.autoNext} disabled={busy} onChange={e => void onOptions({ ...preferences, autoNext: e.target.checked })} />自动连播</label>
      <label><input type="checkbox" checked={preferences.prefetchNext} disabled={busy} onChange={e => void onOptions({ ...preferences, prefetchNext: e.target.checked })} />预取下一集</label>
      <select aria-label="循环模式" value={preferences.repeat} disabled={busy} onChange={e => void onOptions({ ...preferences, repeat: e.target.value as QueueOptions["repeat"] })}>
        <option value="off">顺序播放</option><option value="one">单集循环</option><option value="all">列表循环</option>
      </select>
    </div>
    {!state?.queue.length && <p className="queue-empty">播放磁力视频后，整季视频会按集数排列在这里。</p>}
    <ol className="episode-list">
      {state?.queue.map((file, index) => <li key={file.index}>
        <button className={index === state.queueIndex ? "current" : ""} disabled={busy} onClick={() => void onSelect(index)} aria-current={index === state.queueIndex ? "true" : undefined} title={file.path}>
          <span className="episode-number">{index === state.queueIndex ? "▶" : String(index + 1).padStart(2, "0")}</span>
          <span className="episode-copy"><strong>{file.path.split("/").at(-1)}</strong><small>{formatBytes(file.size)}{state.download?.prefetch?.fileIndex === file.index ? " · 正在预取" : ""}</small></span>
        </button>
      </li>)}
    </ol>
    <p className="queue-hint">仅缓存当前视频和匹配字幕；当前集下载完成后，预取下一集。</p>
  </section>;
}

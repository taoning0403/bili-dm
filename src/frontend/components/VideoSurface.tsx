import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { playerService } from "../services/playerService";
import { getErrorMessage } from "../lib/desktop";
import type { PlaybackState } from "../types/player";
interface Props { state: PlaybackState | null; busy: boolean; onToggle: () => void; onFullscreen: () => void; children?: ReactNode }
export function VideoSurface({ state, busy, onToggle, onFullscreen, children }: Props) {
  const surface = useRef<HTMLDivElement>(null);
  const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [error, setError] = useState("");
  useEffect(() => () => { if (clickTimer.current) clearTimeout(clickTimer.current); }, []);
  useEffect(() => {
    if (!isTauri()) return;
    let frame = 0, active = true, pending = false, dirty = false;
    async function update() {
      if (!active || !surface.current) return;
      if (pending) { dirty = true; return; }
      pending = true;
      const rect = surface.current.getBoundingClientRect();
      const width = window.innerWidth, height = window.innerHeight;
      try {
        await playerService.viewport({
          left: Math.max(0, rect.left / width), right: Math.max(0, (width - rect.right) / width),
          top: Math.max(0, rect.top / height), bottom: Math.max(0, (height - rect.bottom) / height),
        });
        if (active) setError("");
      } catch (cause) { if (active) setError(getErrorMessage(cause)); }
      finally { pending = false; if (dirty && active) { dirty = false; void update(); } }
    }
    const schedule = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(() => void update()); };
    const observer = new ResizeObserver(schedule);
    if (surface.current) observer.observe(surface.current);
    window.addEventListener("resize", schedule); schedule();
    return () => { active = false; observer.disconnect(); cancelAnimationFrame(frame); window.removeEventListener("resize", schedule); };
  }, []);
  const loading = busy || (state?.media && (!state.player.loaded || state.player.buffering));
  return <div ref={surface} className={"video-surface" + (state?.media ? " has-media" : "")}
    onDoubleClick={() => { if (clickTimer.current) clearTimeout(clickTimer.current); onFullscreen(); }}
    onClick={() => {
      if (clickTimer.current) clearTimeout(clickTimer.current);
      if (state?.player.loaded && !busy) clickTimer.current = setTimeout(onToggle, 220);
    }}>
    {!state?.media && <div className="video-empty">
      <div className="play-emblem" aria-hidden="true">▶</div>
      <h1>一部好片，即刻开始</h1><p>粘贴磁力链接，或打开本地视频</p>
      <span>边下载边播放 · 原生画质 · 自动续播</span>
    </div>}
    {children}
    {state?.media && <div className="video-title">{state.media.title.split("/").at(-1)}</div>}
    {loading && <div className="buffering-badge" role="status"><span className="spinner" />{busy ? "正在打开…" : "正在缓冲…"}</div>}
    {state?.player.ended && <div className="buffering-badge">本集播放结束</div>}
    {error && <p className="surface-error" role="alert">{error}</p>}
  </div>;
}

import { useEffect, useRef } from "react";
import type { MixedTrack } from "../types/danmaku";
import type { PlaybackState } from "../types/player";
import { commentX, firstVisibleIndex, playbackTime, scheduleComments } from "../lib/danmakuLayout";

interface Props { track: MixedTrack | null; state: PlaybackState | null; visible: boolean; opacity: number; fontScale: number; busy: boolean }
export function DanmakuOverlay({ track, state, visible, opacity, fontScale, busy }: Props) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const live = useRef({ state, busy }); live.current = { state, busy };
  const aspect = state?.player.videoAspect || 16 / 9;
  useEffect(() => {
    const element = canvas.current;
    if (!element || !track || !visible) return;
    const ctx = element.getContext("2d");
    if (!ctx) return;
    let animation = 0;
    let width = 0, height = 0, left = 0, top = 0;
    let items: ReturnType<typeof scheduleComments> = [];
    function resize() {
      if (!element || !ctx) return;
      const rect = element.getBoundingClientRect(), ratio = window.devicePixelRatio || 1;
      element.width = Math.round(rect.width * ratio); element.height = Math.round(rect.height * ratio);
      width = Math.min(rect.width, rect.height * aspect); height = width / aspect;
      left = (rect.width - width) / 2; top = (rect.height - height) / 2;
      items = scheduleComments(track?.comments ?? [], 1000 / aspect, fontScale, (text, font) => {
        ctx.font = `600 ${font}px "PingFang SC", "Microsoft YaHei", sans-serif`; return ctx.measureText(text).width;
      });
    }
    function draw() {
      if (!ctx || !element) return;
      ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, element.width, element.height);
      const { state: snapshot, busy: opening } = live.current;
      const media = snapshot?.media;
      const key = media?.localPath ?? (media ? `torrent://${media.torrentId}/${media.fileIndex}` : "");
      if (snapshot?.player.loaded && !opening && key === track?.mediaKey && width > 0) {
        const time = playbackTime(snapshot.player, Date.now());
        const ratio = window.devicePixelRatio || 1, scale = width / 1000;
        ctx.setTransform(ratio * scale, 0, 0, ratio * scale, left * ratio, top * ratio);
        ctx.save(); ctx.beginPath(); ctx.rect(0, 0, 1000, height / scale); ctx.clip();
        ctx.globalAlpha = opacity; ctx.textBaseline = "middle"; ctx.lineJoin = "round"; ctx.lineWidth = 2.5; ctx.strokeStyle = "#000";
        for (let index = firstVisibleIndex(items, time); index < items.length; index++) {
          const item = items[index];
          if (!item || item.time > time) break;
          if (item.end <= time) continue;
          ctx.font = `600 ${item.font}px "PingFang SC", "Microsoft YaHei", sans-serif`;
          ctx.fillStyle = `#${item.comment.color.toString(16).padStart(6, "0")}`;
          const text = item.comment.text.slice(0, 300), x = commentX(item, time);
          ctx.strokeText(text, x, item.y); ctx.fillText(text, x, item.y);
        }
        ctx.restore();
      }
      animation = requestAnimationFrame(draw);
    }
    const observer = new ResizeObserver(resize); observer.observe(element); resize(); draw();
    return () => { cancelAnimationFrame(animation); observer.disconnect(); ctx.clearRect(0, 0, element.width, element.height); };
  }, [track, visible, opacity, fontScale, aspect]);
  return <canvas ref={canvas} className="danmaku-overlay" aria-hidden="true" />;
}

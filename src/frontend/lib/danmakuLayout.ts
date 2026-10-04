import type { DanmakuComment } from "../types/danmaku";
import type { PlayerSnapshot } from "../types/player";

export interface ScheduledComment { comment: DanmakuComment; time: number; end: number; width: number; y: number; font: number }
const WIDTH = 1000;
export const SCROLL_SECONDS = 8;

export function playbackTime(player: PlayerSnapshot, now: number): number {
  const elapsed = player.loaded && !player.paused && !player.buffering && !player.ended
    ? Math.max(0, Math.min(1.2, (now - player.sampledAtMs) / 1000)) * player.speed : 0;
  return Math.min(player.duration, Math.max(0, player.position + elapsed));
}
export function scheduleComments(comments: DanmakuComment[], height: number, scale: number, measure: (text: string, font: number) => number): ScheduledComment[] {
  const rowHeight = 34 * scale;
  const count = Math.max(1, Math.min(10, Math.floor(height * 0.55 / rowHeight) - 2));
  const scrolling: (ScheduledComment | undefined)[] = Array(count).fill(undefined);
  const top: (ScheduledComment | undefined)[] = Array(2).fill(undefined);
  const bottom: (ScheduledComment | undefined)[] = Array(2).fill(undefined);
  const result: ScheduledComment[] = [];
  for (const c of comments) {
    const font = Math.max(18, Math.min(40, c.size)) * scale;
    const width = Math.min(4000, measure(c.text.slice(0, 300), font));
    const fixed = c.mode === 4 || c.mode === 5;
    const lanes = c.mode === 4 ? bottom : c.mode === 5 ? top : scrolling;
    const duration = fixed ? 4 : SCROLL_SECONDS;
    const lane = lanes.findIndex(last => {
      if (!last || last.end <= c.time) return true;
      if (fixed || (last.comment.mode === 6) !== (c.mode === 6)) return false;
      const previousSpeed = (WIDTH + last.width) / SCROLL_SECONDS;
      const speed = (WIDTH + width) / SCROLL_SECONDS;
      const entered = c.time - last.time >= (last.width + 20) / previousSpeed;
      const noCatchUp = speed <= previousSpeed || WIDTH - (last.end - c.time) * speed >= 20;
      return entered && noCatchUp;
    });
    if (lane < 0) continue; // Density reduction only affects display, never the exported file.
    const y = c.mode === 4 ? height * 0.86 - lane * rowHeight : (c.mode === 5 ? lane + 1 : lane + 3) * rowHeight;
    const item = { comment: c, time: c.time, end: c.time + duration, width, y, font };
    lanes[lane] = item; result.push(item);
  }
  return result;
}
export function firstVisibleIndex(items: ScheduledComment[], time: number): number {
  let low = 0, high = items.length;
  while (low < high) { const middle = (low + high) >>> 1; if ((items[middle]?.time ?? 0) < time - SCROLL_SECONDS) low = middle + 1; else high = middle; }
  return low;
}
export function commentX(item: ScheduledComment, time: number): number {
  if (item.comment.mode === 4 || item.comment.mode === 5) return (WIDTH - item.width) / 2;
  const progress = (time - item.time) / (item.end - item.time);
  return item.comment.mode === 6 ? -item.width + progress * (WIDTH + item.width) : WIDTH - progress * (WIDTH + item.width);
}

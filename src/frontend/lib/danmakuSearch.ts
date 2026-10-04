import type { MatchOptions } from "../types/danmaku.ts";
export const defaultMatchOptions: MatchOptions = {
  mode: "hybrid", allowEdits: false, allowRedraw: true, allowAudioOnly: false,
  minDurationRatio: 1, minCoverage: 0.8, minSegment: 4, maxError: 0.5, sampleStep: 2,
  maxCandidates: 20, budgetSeconds: 600, audioTrackId: null,
};
export function searchTitle(title: string): string {
  return title.split(/[\\/]/).pop()!.replace(/\.[a-z0-9]{2,5}$/i, "")
    .replace(/^\[[a-z0-9 _.-]+\]\s*/i, "")
    .replace(/\bS(\d{1,2})E(\d{1,3})\b/ig, (_, season: string, episode: string) => ` 第${Number(season)}季 第${Number(episode)}集 `)
    .replace(/\b(?:EP|E)(\d{1,3})\b/ig, (_, episode: string) => ` 第${Number(episode)}集 `)
    .replace(/\b(?:\d{3,4}[pi]|[248]k|x26[45]|h\.?26[45]|hevc|avc|av1|bluray|blu-ray|web-dl|webrip|bdrip|dvdrip|aac(?:\d[.\d]*)?|flac|dts(?:-hd)?|truehd|hdr10?\+?|dolby|10bit|8bit)\b/ig, " ")
    .replace(/[._]/g, " ").replace(/[\[\]()]/g, " ").replace(/\s+/g, " ").trim();
}
export function readMatchOptions(): MatchOptions {
  try { const saved: unknown = JSON.parse(localStorage.getItem("bili-dm.match-options") ?? "null"); if (saved && typeof saved === "object") return { ...defaultMatchOptions, ...saved, audioTrackId: null }; } catch { /* Start with defaults if settings are unavailable. */ }
  return { ...defaultMatchOptions };
}

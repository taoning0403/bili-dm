import { invokeDesktop } from "../lib/desktop";
import type { DanmakuClip, DanmakuJob, DanmakuWorkspace, MatchResult, MatchOptions, SearchResult, AlignmentPreview } from "../types/danmaku";

export const danmakuService = {
  workspace: (sessionId: string) => invokeDesktop<DanmakuWorkspace>("get_danmaku_workspace", { sessionId }, 0),
  resolve: (sessionId: string, inputs: string[]) => invokeDesktop<DanmakuWorkspace>("resolve_danmaku", { sessionId, inputs }, 0),
  match: (sessionId: string, sourceIds: string[], options: MatchOptions) => invokeDesktop<MatchResult>("match_danmaku", { sessionId, sourceIds, options }, 0),
  search: (sessionId: string, query: string, page: number, options: MatchOptions) => invokeDesktop<SearchResult>("search_danmaku", { sessionId, query, page, options }, 0),
  preview: (sessionId: string, sourceId: string, sourceTime: number, targetTime: number) => invokeDesktop<AlignmentPreview>("preview_danmaku", { sessionId, sourceId, sourceTime, targetTime }, 0),
  apply: (sessionId: string, clips: DanmakuClip[]) => invokeDesktop<DanmakuWorkspace>("apply_danmaku", { sessionId, clips }, 0),
  export: (sessionId: string) => invokeDesktop<string | null>("export_danmaku", { sessionId }, 0),
  job: () => invokeDesktop<DanmakuJob>("get_danmaku_job"),
  cancel: () => invokeDesktop<void>("cancel_danmaku"),
};

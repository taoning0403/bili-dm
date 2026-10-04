import { invokeDesktop } from "../lib/desktop";
import type { DanmakuClip, DanmakuJob, DanmakuWorkspace, MatchResult } from "../types/danmaku";

export const danmakuService = {
  workspace: (sessionId: string) => invokeDesktop<DanmakuWorkspace>("get_danmaku_workspace", { sessionId }, 0),
  resolve: (sessionId: string, inputs: string[]) => invokeDesktop<DanmakuWorkspace>("resolve_danmaku", { sessionId, inputs }, 0),
  match: (sessionId: string, sourceIds: string[]) => invokeDesktop<MatchResult>("match_danmaku", { sessionId, sourceIds }, 0),
  apply: (sessionId: string, clips: DanmakuClip[]) => invokeDesktop<DanmakuWorkspace>("apply_danmaku", { sessionId, clips }, 0),
  export: (sessionId: string) => invokeDesktop<string | null>("export_danmaku", { sessionId }, 0),
  job: () => invokeDesktop<DanmakuJob>("get_danmaku_job"),
  cancel: () => invokeDesktop<void>("cancel_danmaku"),
};

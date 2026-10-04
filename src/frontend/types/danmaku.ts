export interface DanmakuComment { id: string; time: number; mode: number; color: number; size: number; text: string }
export interface DanmakuSource { id: string; bvid: string; cid: number; page: number; title: string; duration: number; commentCount: number; warnings: string[] }
export interface DanmakuClip {
  id: string; sourceId: string; sourceStart: number; sourceEnd: number; targetStart: number; targetEnd: number;
  enabled: boolean; confidence: number | null; evidence: string | null;
}
export interface MixedTrack { mediaKey: string; duration: number; comments: DanmakuComment[]; warnings: string[]; filePath: string }
export interface DanmakuWorkspace { mediaKey: string; sources: DanmakuSource[]; clips: DanmakuClip[]; mixed: MixedTrack | null; errors: string[] }
export interface DanmakuJob { running: boolean; message: string; completed: number; total: number }
export interface MatchResult { clips: DanmakuClip[]; errors: string[] }

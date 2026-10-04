export interface DanmakuComment { id: string; time: number; mode: number; color: number; size: number; text: string }
export interface DanmakuSource { id: string; bvid: string; cid: number; page: number; title: string; duration: number; commentCount: number; warnings: string[]; episodeId: number | null }
export interface DanmakuClip {
  id: string; sourceId: string; sourceStart: number; sourceEnd: number; targetStart: number; targetEnd: number;
  enabled: boolean; confidence: number | null; evidence: string | null;
  reviewRequired: boolean; evidenceKind: "video" | "audio" | "both" | null;
}
export interface MixedTrack { mediaKey: string; duration: number; comments: DanmakuComment[]; warnings: string[]; filePath: string }
export interface DanmakuWorkspace { mediaKey: string; sources: DanmakuSource[]; clips: DanmakuClip[]; mixed: MixedTrack | null; errors: string[] }
export interface DanmakuJob { running: boolean; message: string; completed: number; total: number }
export interface MatchResult { clips: DanmakuClip[]; errors: string[] }
export interface MatchOptions {
  mode: "hybrid" | "video" | "audio"; allowEdits: boolean; allowRedraw: boolean; allowAudioOnly: boolean;
  minDurationRatio: number; minCoverage: number; minSegment: number; maxError: number; sampleStep: number;
  maxCandidates: number; budgetSeconds: number; audioTrackId: number | null;
}
export interface SearchCandidate { title: string; input: string; duration: number | null; state: string; reason: string; coverage: number; clipCount: number }
export interface SearchResult { workspace: DanmakuWorkspace; clips: DanmakuClip[]; candidates: SearchCandidate[]; errors: string[]; page: number; hasMore: boolean }

export interface AlignmentPreview { source: { dataUrl: string; time: number }; target: { dataUrl: string; time: number } }

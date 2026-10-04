import { useEffect, useRef, useState } from "react";
import { danmakuService } from "../services/danmakuService";
import { getErrorMessage } from "../lib/desktop";
import type { PlaybackState } from "../types/player";
import type { DanmakuClip, DanmakuJob, DanmakuSource, DanmakuWorkspace } from "../types/danmaku";

export function useDanmaku(playback: PlaybackState | null) {
  const session = playback?.player.loaded && playback.player.duration > 0 ? playback.media?.sessionId ?? "" : "";
  const [workspace, setWorkspace] = useState<DanmakuWorkspace | null>(null);
  const [clips, setClips] = useState<DanmakuClip[]>([]);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(false);
  const [job, setJob] = useState<DanmakuJob | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [notice, setNotice] = useState("");
  const [dirty, setDirty] = useState(false);
  const [visible, setVisible] = useState(true);
  const [opacity, setOpacity] = useState(0.85);
  const [fontScale, setFontScale] = useState(1);
  const current = useRef(session); current.current = session;
  const inFlight = useRef(false);
  const revision = useRef(0);
  useEffect(() => {
    const ticket = ++revision.current;
    let active = true;
    setWorkspace(null); setClips([]); setErrors([]); setNotice(""); setDirty(false);
    if (!session) { setLoading(false); return; }
    setLoading(true);
    void danmakuService.workspace(session).then(next => {
      if (active && ticket === revision.current) { setWorkspace(next); setClips(next.clips); }
    }).catch(cause => { if (active) setErrors([getErrorMessage(cause)]); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [session]);
  useEffect(() => {
    if (!busy) { setJob(null); return; }
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try { const next = await danmakuService.job(); if (active) setJob(next); }
      catch { /* The operation itself reports transport errors. */ }
      if (active) timer = setTimeout(() => void poll(), 500);
    }
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, [busy]);
  async function run<T>(work: (id: string) => Promise<T>, done: (value: T) => void) {
    if (!session || inFlight.current || loading) return;
    const id = session, ticket = revision.current;
    inFlight.current = true; setBusy(true); setErrors([]); setNotice("");
    try { const value = await work(id); if (current.current === id && revision.current === ticket) done(value); }
    catch (cause) { if (current.current === id && revision.current === ticket) setErrors([getErrorMessage(cause)]); }
    finally { inFlight.current = false; setBusy(false); }
  }
  function edit(next: DanmakuClip[]) { setClips(next); setDirty(true); }
  return {
    workspace, clips, busy: busy || loading, job, errors, notice, dirty, visible, setVisible, opacity, setOpacity, fontScale, setFontScale,
    ready: !!session, track: workspace?.mixed ?? null,
    resolve: (input: string) => run(id => danmakuService.resolve(id, input.split(/\r?\n/).map(v => v.trim()).filter(Boolean)), next => { setWorkspace(next); setErrors(next.errors); setNotice(`已解析 ${next.sources.length} 套弹幕；添加区间后生成混合文件。`); }),
    match: (ids: string[]) => run(id => danmakuService.match(id, ids), next => {
      edit([...clips, ...next.clips]); setErrors(next.errors);
      setNotice(`新增 ${next.clips.length} 个匹配区间，请检查时间后生成。`);
    }),
    apply: () => {
      if (clips.some(c => c.enabled && [c.sourceStart, c.sourceEnd, c.targetStart, c.targetEnd].some(v => !Number.isFinite(v)))) { setErrors(["请为启用的区间填写完整、有效的时间。"]); return Promise.resolve(); }
      return run(id => danmakuService.apply(id, clips), next => { setWorkspace(next); setClips(next.clips); setErrors(next.errors); setDirty(false); setVisible(true); setNotice("已保存混合文件，当前视频开始显示弹幕。"); });
    },
    export: () => run(danmakuService.export, path => { if (path) setNotice(`已导出：${path}`); }),
    cancel: async () => { try { await danmakuService.cancel(); } catch (cause) { setErrors([getErrorMessage(cause)]); } },
    update: (id: string, patch: Partial<DanmakuClip>) => edit(clips.map(c => c.id === id ? { ...c, ...patch, confidence: null, evidence: "手动调整；生成时校验时长和边界。" } : c)),
    remove: (id: string) => edit(clips.filter(c => c.id !== id)),
    add: (source: DanmakuSource) => {
      const duration = Math.min(source.duration, playback?.player.duration ?? source.duration);
      edit([...clips, { id: crypto.randomUUID(), sourceId: source.id, sourceStart: 0, sourceEnd: duration, targetStart: 0, targetEnd: duration, enabled: true, confidence: null, evidence: null }]);
    },
  };
}
export type DanmakuController = ReturnType<typeof useDanmaku>;

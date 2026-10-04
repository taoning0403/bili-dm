import type { DanmakuController } from "../hooks/useDanmaku";
import type { MatchOptions } from "../types/danmaku";
import { defaultMatchOptions } from "../lib/danmakuSearch";
import { formatTime } from "../lib/format";
const states: Record<string,string> = { matched:"已匹配", review:"待确认", filtered:"时长不符", unavailable:"不可用", unmatched:"未匹配", limit:"达到上限" };
export function DanmakuSearch({ dm }: { dm: DanmakuController }) {
  function change(patch: Partial<MatchOptions>) { dm.setOptions(previous => ({ ...previous, ...patch })); }
  return <section className="dm-section dm-search">
    <h2>按标题智能搜索</h2>
    <label className="dm-input-label" htmlFor="danmaku-query">片名 / 季数 / 集数</label>
    <input id="danmaku-query" value={dm.query} disabled={dm.busy} onChange={e => dm.setQuery(e.target.value)} placeholder="自动提取当前视频标题，可编辑" />
    <fieldset disabled={dm.busy} className="dm-strategy">
      <legend>搜索与匹配策略</legend>
      <div className="dm-clip-actions"><button onClick={() => dm.setOptions({ ...defaultMatchOptions })}>标准</button><button onClick={() => change({ allowEdits: true, allowRedraw: true, minDurationRatio: 0.85, mode: "hybrid" })}>兼容删减与重绘</button></div>
      <label>匹配依据 <select aria-label="匹配依据" value={dm.options.mode} onChange={e => change({ mode: e.target.value as MatchOptions["mode"] })}><option value="hybrid">结合画面与音频</option><option value="video">仅画面</option><option value="audio">仅音频</option></select></label>
      <label><input type="checkbox" checked={dm.options.allowEdits} onChange={e => change({ allowEdits: e.target.checked })} /> 允许中间删减 / 插入</label>
      <label><input type="checkbox" checked={dm.options.allowRedraw} onChange={e => change({ allowRedraw: e.target.checked })} /> 允许音频补充重绘画面</label>
      <div className="dm-range-grid">
        <label>最低候选时长（%）<input aria-label="最低候选时长比例" type="number" min="0" max="100" step="1" value={dm.options.minDurationRatio * 100} onChange={e => change({ minDurationRatio: e.target.valueAsNumber / 100 })} /></label>
        <label>最低覆盖率（%）<input aria-label="最低匹配覆盖率" type="number" min="10" max="100" step="1" value={dm.options.minCoverage * 100} onChange={e => change({ minCoverage: e.target.valueAsNumber / 100 })} /></label>
      </div>
      <p className="muted">时长比例 100% 会过滤短版本；删减版可降低此值。覆盖不足和证据冲突的结果会保留为待确认区间。</p>
      <details><summary>高级策略</summary>
        <label>分析音轨 <select aria-label="分析音轨" value={dm.options.audioTrackId ?? ""} onChange={e => change({ audioTrackId: e.target.value ? Number(e.target.value) : null })}><option value="">使用当前所选音轨</option>{dm.audioTracks.map(t => <option key={t.id} value={t.id}>{t.title || t.language || `音轨 ${t.id}`}</option>)}</select></label>
        <label><input type="checkbox" checked={dm.options.allowAudioOnly} onChange={e => change({ allowAudioOnly: e.target.checked })} /> 仅有音频证据也自动启用</label>
        <p className="muted">共用配乐可能产生相似结果，默认需要预览确认。</p>
        <div className="dm-range-grid">{([
          ["minSegment","最短可信区间（秒）",1,60,1], ["maxError","时间误差容限（秒）",0.1,2,0.1], ["sampleStep","画面采样间隔（秒）",0.5,10,0.5], ["maxCandidates","本页候选上限",1,50,1], ["budgetSeconds","单次分析预算（秒）",30,3600,30],
        ] as const).map(([key,label,min,max,step]) => <label key={key}>{label}<input type="number" aria-label={label} min={min} max={max} step={step} value={Number.isFinite(dm.options[key]) ? dm.options[key] : ""} onChange={e => change({ [key]: e.target.valueAsNumber })} /></label>)}</div>
      </details>
    </fieldset>
    <button className="primary-button play-selected" disabled={dm.busy || !dm.ready || !dm.query.trim()} onClick={() => void dm.search()}>搜索并逐一匹配</button>
    {!!dm.candidates.length && <details open className="dm-candidates"><summary>候选结果 · {dm.candidates.length}</summary>{dm.candidates.map((candidate,index) => <div key={`${candidate.input}:${index}`} className="dm-candidate"><div><strong>{candidate.title}</strong><span className={`dm-status ${candidate.state}`}>{states[candidate.state] ?? candidate.state}</span></div><p className="muted">{candidate.duration !== null ? formatTime(candidate.duration) : "时长未知"}{candidate.clipCount > 0 ? ` · 覆盖 ${(candidate.coverage * 100).toFixed(1)}% · ${candidate.clipCount} 段` : ""}</p>{candidate.reason && <p className="muted">{candidate.reason}</p>}</div>)}</details>}
    {dm.hasMore && <button disabled={dm.busy} onClick={() => void dm.search(dm.searchPage + 1)}>继续搜索下一页</button>}
  </section>;
}

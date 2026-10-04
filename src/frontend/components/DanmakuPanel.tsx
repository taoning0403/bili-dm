import { BilibiliAccount } from "./BilibiliAccount";
import { DanmakuSearch } from "./DanmakuSearch";
import { DanmakuTimeline } from "./DanmakuTimeline";
import { useEffect, useState } from "react";
import type { DanmakuController } from "../hooks/useDanmaku";
import { formatTime } from "../lib/format";

interface Props { controller: DanmakuController; duration: number; position: number; onSeek: (seconds: number) => void }
export function DanmakuPanel({ controller: dm, duration, position, onSeek }: Props) {
  const [input, setInput] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const sources = dm.workspace?.sources;
  useEffect(() => { setSelected(sources?.map(s => s.id) ?? []); }, [sources]);
  return <div className="danmaku-panel">
    <div className="section-heading"><h2>哔哩哔哩弹幕混合</h2><span>{duration > 0 ? formatTime(duration) : "未打开视频"}</span></div>
    <BilibiliAccount />
    <DanmakuSearch dm={dm} />
    {!dm.ready && <p className="queue-hint">先播放本地或磁力视频，再为当前视频配置弹幕。</p>}
    <p className="muted">可合并多个视频的弹幕。按源视频区间截取，再映射到当前视频；音画分段匹配结果可手动调整。</p>
    <label className="dm-input-label" htmlFor="bilibili-links">BV / ep / ss 或视频链接，每行一个</label>
    <textarea id="bilibili-links" value={input} onChange={event => setInput(event.target.value)} placeholder={"BV1xx411c7mD\nhttps://www.bilibili.com/video/BV…?p=2"} disabled={dm.busy} />
    <p className="muted">不带 p 参数将读取全部分 P；每批最多 20 个链接、50 套弹幕。</p>
    <button className="primary-button play-selected" disabled={!dm.ready || dm.busy || !input.trim()} onClick={() => void dm.resolve(input)}>批量解析视频与弹幕</button>
    {dm.busy && <div className="dm-progress" role="status"><span>{dm.job?.message || "正在处理…"}</span>
      {!!dm.job?.total && <progress value={dm.job.completed} max={dm.job.total} />}
      <button onClick={() => void dm.cancel()}>取消任务</button></div>}
    {!!sources?.length && <section className="dm-section">
      <h2>弹幕源 <small>{sources.length}</small></h2>
      <div className="dm-source-list">{sources.map(source => <div className="dm-source" key={source.id}>
        <label><input type="checkbox" checked={selected.includes(source.id)} disabled={dm.busy} onChange={event => setSelected(event.target.checked ? [...selected, source.id] : selected.filter(id => id !== source.id))} /><strong>{source.title}</strong></label>
        <p>{source.episodeId ? `ep${source.episodeId}` : source.bvid} · {source.episodeId ? "第" : "P"}{source.page} · {formatTime(source.duration)} · {source.commentCount.toLocaleString()} 条</p>
        {source.warnings.map(w => <p className="dm-warning" key={w}>{w}</p>)}
        <button disabled={dm.busy} onClick={() => dm.add(source)}>＋ 手动添加区间</button>
      </div>)}</div>
      <button className="play-selected" disabled={dm.busy || !selected.length} onClick={() => void dm.match(selected)}>智能匹配所选弹幕源</button>
      <p className="muted">分析会读取视频各处的画面和音轨，磁力视频可能需要等待分片。低覆盖率、音画冲突和缺少前后画面支持的纯音频结果默认待确认。</p>
    </section>}
    {!!dm.clips.length && <section className="dm-section">
      <h2>区间编辑 <small>{dm.clips.length}</small></h2>
      <p className="muted">时间单位：秒，支持小数。结束时间不含该时刻；两段时长不同时按比例映射。可重复添加同一弹幕源的不同区间。</p>
      {dm.clips.map((clip, index) => {
        const source = sources?.find(s => s.id === clip.sourceId);
        const sourceLength = clip.sourceEnd - clip.sourceStart, targetLength = clip.targetEnd - clip.targetStart;
        return <fieldset className="dm-clip" key={clip.id} disabled={dm.busy}>
          <legend><label><input type="checkbox" checked={clip.enabled} onChange={e => dm.update(clip.id, { enabled: e.target.checked })} /> 区间 {index + 1}</label></legend>
          <p className="dm-clip-title">{source?.title ?? "弹幕源已失效"}</p>
          <DanmakuTimeline clip={clip} sourceDuration={source?.duration ?? duration} targetDuration={duration} onChange={patch => dm.update(clip.id, patch)} onSeek={onSeek} />
          <div className="dm-range-grid">
            {([['sourceStart', '源开始', source?.duration], ['sourceEnd', '源结束', source?.duration], ['targetStart', '当前开始', duration], ['targetEnd', '当前结束', duration]] as const).map(([key, label, max]) => <label key={key}>{label}
              <input aria-label={`区间 ${index + 1} ${label}`} type="number" min="0" max={max} step="0.01" value={Number.isFinite(clip[key]) ? Math.round(clip[key] * 100) / 100 : ""} onChange={e => dm.update(clip.id, { [key]: e.target.valueAsNumber })} />
            </label>)}
          </div>
          <p className="muted">源 {sourceLength.toFixed(2)}s → 当前 {targetLength.toFixed(2)}s{Math.abs(sourceLength - targetLength) > 0.05 ? " · 将按比例同步" : " · 等时长"}</p>
          {clip.reviewRequired && <p className="dm-warning">待确认：预览并调整后勾选此区间。</p>}
          {clip.confidence !== null && <p className="dm-match-score">证据得分 {Math.round(clip.confidence * 100)}%</p>}
          {clip.evidence && <p className="muted">{clip.evidence}</p>}
          {dm.preview?.clipId === clip.id && <div className="dm-preview-pair">{([['source','B 站来源'],['target','当前视频']] as const).map(([key,label]) => <figure key={key}><img src={dm.preview!.frames[key].dataUrl} alt={`${label}对应画面`} /><figcaption>{label} · {dm.preview!.frames[key].time.toFixed(2)}s</figcaption></figure>)}<button onClick={dm.closePreview}>关闭画面对照</button></div>}
          <div className="dm-clip-actions"><button disabled={[clip.sourceStart,clip.sourceEnd,clip.targetStart,clip.targetEnd].some(v => !Number.isFinite(v))} onClick={() => void dm.previewClip(clip, position >= clip.targetStart && position < clip.targetEnd ? position : (clip.targetStart + clip.targetEnd) / 2)}>对照预览</button><button disabled={!Number.isFinite(clip.targetStart)} onClick={() => onSeek(clip.targetStart)}>定位开始</button><button onClick={() => dm.update(clip.id, { targetStart: position })}>当前进度作起点</button><button disabled={position <= clip.targetStart || position >= clip.targetEnd} onClick={() => dm.split(clip.id, position)}>在当前进度拆分</button><button onClick={() => dm.remove(clip.id)}>移除</button></div>
        </fieldset>;
      })}
      <button className="primary-button play-selected" disabled={dm.busy || !dm.clips.some(c => c.enabled)} onClick={() => void dm.apply()}>生成混合文件并播放</button>
      {dm.dirty && <p className="dm-warning">区间尚未生成保存；播放器继续使用上次混合结果。</p>}
    </section>}
    {dm.track && <section className="dm-section dm-display-settings">
      <h2>弹幕显示 <small>{dm.track.comments.length.toLocaleString()} 条</small></h2>
      <label><input type="checkbox" checked={dm.visible} onChange={e => dm.setVisible(e.target.checked)} /> 显示弹幕</label>
      <label>不透明度 <input aria-label="弹幕不透明度" type="range" min="0.2" max="1" step="0.05" value={dm.opacity} onChange={e => dm.setOpacity(Number(e.target.value))} /></label>
      <label>字号 <input aria-label="弹幕字号" type="range" min="0.7" max="1.5" step="0.1" value={dm.fontScale} onChange={e => dm.setFontScale(Number(e.target.value))} /></label>
      <p className="muted">密集时减少屏幕上的碰撞弹幕；导出文件保留全部混合结果。弹幕不占用字幕轨。</p>
      <button disabled={dm.busy} onClick={() => void dm.export()}>导出混合 XML</button>
      {dm.track.warnings.map(w => <p className="dm-warning" key={w}>{w}</p>)}
    </section>}
    {dm.notice && <p className="dm-notice" role="status">{dm.notice}</p>}
    {!!dm.errors.length && <div className="error-message" role="alert">{dm.errors.map((error, i) => <p key={i}>{error}</p>)}</div>}
  </div>;
}

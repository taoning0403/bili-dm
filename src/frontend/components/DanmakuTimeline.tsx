import type { DanmakuClip } from "../types/danmaku";
import { formatTime } from "../lib/format";
interface Props { clip: DanmakuClip; sourceDuration: number; targetDuration: number; onChange: (patch: Partial<DanmakuClip>) => void; onSeek: (time: number) => void }
export function DanmakuTimeline({ clip, sourceDuration, targetDuration, onChange, onSeek }: Props) {
  return <div className="dm-timelines">{([
    ["sourceStart","sourceEnd",sourceDuration,"B 站来源"], ["targetStart","targetEnd",targetDuration,"当前视频"],
  ] as const).map(([start,end,duration,label]) => <div key={start}>
    <div className="dm-timeline-label"><span>{label}</span><span>{formatTime(clip[start])} – {formatTime(clip[end])}</span></div>
    <div className={`dm-timeline ${clip.reviewRequired ? "review" : ""}`}><button title={start === "targetStart" ? "定位当前区间" : "对应的来源区间"} onClick={() => onSeek(clip.targetStart)} style={{ left: `${clip[start] / duration * 100}%`, width: `${(clip[end] - clip[start]) / duration * 100}%` }} /></div>
    <div className="dm-timeline-handles"><input aria-label={`${label}开始边界`} type="range" min="0" max={duration} step="0.1" value={clip[start]} onChange={e => onChange({ [start]: Math.min(Number(e.target.value),clip[end]-0.1) })} /><input aria-label={`${label}结束边界`} type="range" min="0" max={duration} step="0.1" value={clip[end]} onChange={e => onChange({ [end]: Math.max(Number(e.target.value),clip[start]+0.1) })} /></div>
  </div>)}</div>;
}

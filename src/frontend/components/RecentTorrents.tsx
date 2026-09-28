import type { TorrentRecord } from "../types/library";

interface Props { tasks: TorrentRecord[]; disabled: boolean; onLoad: (magnet: string) => Promise<void> }

export function RecentTorrents({ tasks, disabled, onLoad }: Props) {
  if (!tasks.length) return null;
  return <details className="recent-panel">
    <summary>最近加载的磁力 · {tasks.length}</summary>
    <p className="muted">保存在本机，点击重新加载目录。</p>
    <ul>
      {tasks.map((task) => <li key={task.torrentId}>
        <button disabled={disabled} onClick={() => void onLoad(task.magnetUri)}>{task.name}</button>
        <span className="muted">{task.fileCount} 个文件 · {({ ready: "已解析", downloading: "下载中", paused: "已停止下载" })[task.status]}</span>
      </li>)}
    </ul>
  </details>;
}

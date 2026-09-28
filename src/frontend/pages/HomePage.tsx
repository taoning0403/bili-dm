import { AppHeader } from "../components/AppHeader";
import { MagnetInput } from "../components/MagnetInput";
import { useTorrent } from "../hooks/useTorrent";
import { formatBytes } from "../lib/format";

export function HomePage() {
  const torrent = useTorrent();

  return (
    <div className="app-shell">
      <AppHeader />
      <main>
        <MagnetInput loading={torrent.loading} onLoad={torrent.load} onCancel={torrent.cancel} />
        {torrent.loading && <p role="status">正在从节点读取元数据…</p>}
        {torrent.error && <p className="error-message" role="alert">{torrent.error}</p>}
        {torrent.catalog && <section className="file-panel">
          <h2>{torrent.catalog.name}</h2>
          <p className="muted">{torrent.catalog.files.length} 个文件</p>
          <ul className="file-list">{torrent.catalog.files.map((file) => <li key={file.index}>
            <span className="file-path">{file.path}</span><span>{formatBytes(file.size)}</span>
          </li>)}</ul>
        </section>}
      </main>
      <footer className="app-footer">无需账号 · 本地优先 · torrent 数据保存在本机缓存目录</footer>
    </div>
  );
}

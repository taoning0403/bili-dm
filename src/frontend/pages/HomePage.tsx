import { AppHeader } from "../components/AppHeader";
import { MagnetInput } from "../components/MagnetInput";
import { useTorrent } from "../hooks/useTorrent";
import { FileCatalog } from "../components/FileCatalog";

export function HomePage() {
  const torrent = useTorrent();

  return (
    <div className="app-shell">
      <AppHeader />
      <main>
        <MagnetInput loading={torrent.loading} onLoad={torrent.load} onCancel={torrent.cancel} />
        {torrent.loading && <p role="status">正在从节点读取元数据…</p>}
        {torrent.error && <p className="error-message" role="alert">{torrent.error}</p>}
        {torrent.catalog && <FileCatalog catalog={torrent.catalog} selected={torrent.selectedFile} onSelect={torrent.selectFile} />}
      </main>
      <footer className="app-footer">无需账号 · 本地优先 · torrent 数据保存在本机缓存目录</footer>
    </div>
  );
}

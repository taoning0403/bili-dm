import { useState } from "react";

interface Props {
  loading: boolean;
  onLoad: (magnet: string) => Promise<void>;
  onCancel: () => Promise<void>;
}

export function MagnetInput({ loading, onLoad, onCancel }: Props) {
  const [magnet, setMagnet] = useState("");
  return (
    <form className="source-panel" onSubmit={(event) => { event.preventDefault(); void onLoad(magnet); }}>
      <label htmlFor="magnet">磁力链接</label>
      <textarea id="magnet" placeholder="magnet:?xt=urn:btih:…" value={magnet}
        onChange={(event) => setMagnet(event.target.value)} disabled={loading} rows={2} spellCheck={false} />
      <div className="source-actions">
        <p className="muted">先读取文件目录，选择视频后才开始下载。解析最长等待 120 秒。</p>
        {loading ? <button type="button" onClick={() => void onCancel()}>取消解析</button>
          : <button className="primary-button" type="submit" disabled={!magnet.trim()}>加载目录</button>}
      </div>
    </form>
  );
}

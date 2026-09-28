interface Props {
  magnet: string;
  onChange: (magnet: string) => void;
  loading: boolean;
  onLoad: (magnet: string) => Promise<void>;
  onCancel: () => Promise<void>;
}

export function MagnetInput({ magnet, onChange, loading, onLoad, onCancel }: Props) {
  return (
    <form className="source-panel" onSubmit={(event) => { event.preventDefault(); void onLoad(magnet); }}>
      <label htmlFor="magnet">磁力链接</label>
      <textarea id="magnet" placeholder="magnet:?xt=urn:btih:…" value={magnet}
        onChange={(event) => onChange(event.target.value)} disabled={loading} rows={2} spellCheck={false} />
      <div className="source-actions">
        <p className="muted">先读取文件目录，选择视频后才开始下载。解析最长等待 120 秒。</p>
        {loading ? <button type="button" onClick={() => void onCancel()}>取消解析</button>
          : <button className="primary-button" type="submit" disabled={!magnet.trim()}>加载目录</button>}
      </div>
    </form>
  );
}

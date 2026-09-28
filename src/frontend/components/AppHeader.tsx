export function AppHeader() {
  return (
    <header className="app-header">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">▶</span>
        <div>
          <p className="brand-name">Bili DM</p>
          <p className="brand-caption">本地媒体播放器</p>
        </div>
      </div>
      <span className="phase-label">磁力目录</span>
    </header>
  );
}

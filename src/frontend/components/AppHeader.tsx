export function AppHeader({ onOpenLocal, busy }: { onOpenLocal: () => Promise<void>; busy: boolean }) {
  return (
    <header className="app-header">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">▶</span>
        <div>
          <p className="brand-name">Bili DM</p>
          <p className="brand-caption">本地媒体播放器</p>
        </div>
      </div>
      <button onClick={() => void onOpenLocal()} disabled={busy}>打开本地视频</button>
    </header>
  );
}

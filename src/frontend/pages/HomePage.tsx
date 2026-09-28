import { AppHeader } from "../components/AppHeader";
import { RuntimeStatus } from "../components/RuntimeStatus";
import { useRuntimeInfo } from "../hooks/useRuntimeInfo";

export function HomePage() {
  const { state, refresh } = useRuntimeInfo();

  return (
    <div className="app-shell">
      <AppHeader />
      <main>
        <section className="welcome-panel" aria-labelledby="welcome-title">
          <div className="video-placeholder" aria-hidden="true">
            <span className="play-outline">▶</span>
          </div>
          <div className="welcome-copy">
            <p className="eyebrow">LOCAL FIRST</p>
            <h1 id="welcome-title">你的媒体，留在本地。</h1>
            <p className="muted">播放器正在起步。当前版本提供桌面应用框架，暂未开放媒体播放。</p>
            <p className="next-step">下一阶段：从磁力链接读取文件列表。</p>
          </div>
        </section>
        <RuntimeStatus state={state} onRefresh={refresh} />
      </main>
      <footer className="app-footer">无需账号 · 本地优先</footer>
    </div>
  );
}

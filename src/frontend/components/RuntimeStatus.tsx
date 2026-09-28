import type { RuntimeState } from "../types/runtime";

interface RuntimeStatusProps {
  state: RuntimeState;
  onRefresh: () => void;
}

export function RuntimeStatus({ state, onRefresh }: RuntimeStatusProps) {
  return (
    <section className="runtime-panel" aria-labelledby="runtime-title">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">DESKTOP STATUS</p>
          <h2 id="runtime-title">运行状态</h2>
        </div>
        <button
          className="secondary-button"
          type="button"
          onClick={onRefresh}
          disabled={state.status === "checking"}
        >
          {state.status === "checking" ? "检查中…" : "重新检查"}
        </button>
      </div>
      <div role="status" aria-live="polite" aria-atomic="true">
        {state.status === "checking" && <p className="muted">正在连接桌面服务…</p>}
        {state.status === "error" && <p className="error-message">{state.message}</p>}
        {state.status === "ready" && (
          <>
            <p className="connection-status">
              <span className="status-dot" aria-hidden="true" />
              桌面服务已连接
            </p>
            <dl className="runtime-details">
              <div><dt>应用</dt><dd>{state.info.appName}</dd></div>
              <div><dt>版本</dt><dd>{state.info.appVersion}</dd></div>
              <div><dt>系统</dt><dd>{state.info.platform}</dd></div>
              <div><dt>架构</dt><dd>{state.info.architecture}</dd></div>
            </dl>
          </>
        )}
      </div>
    </section>
  );
}

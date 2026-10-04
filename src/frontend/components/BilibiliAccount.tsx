import { useBilibiliAccount } from "../hooks/useBilibiliAccount";
export function BilibiliAccount() {
  const account = useBilibiliAccount();
  const user = account.status.account;
  const width = account.qr?.pixels.length ?? 0;
  return <section className="dm-section dm-account">
    <div className="section-heading"><h2>B 站账号</h2><span>{user ? user.vip ? "大会员" : "已保存登录" : "未登录"}</span></div>
    {user && <p><strong>{user.name}</strong> · UID {user.uid}{account.status.state === "saved" ? " · 待验证" : ""}</p>}
    <div className="dm-clip-actions">
      <button disabled={account.busy} onClick={() => void account.start()}>{user ? "切换账号 / 重新扫码" : "扫码登录"}</button>
      {user && <><button disabled={account.busy || !!account.qr} onClick={() => void account.verify()}>验证登录</button><button disabled={account.busy} onClick={() => void account.logout()}>退出并清除凭据</button></>}
    </div>
    {account.qr && <div className="dm-login-qr">
      <svg role="img" aria-label="B 站登录二维码" viewBox={`-4 -4 ${width + 8} ${width + 8}`} shapeRendering="crispEdges"><rect x="-4" y="-4" width={width + 8} height={width + 8} fill="white" />{account.qr.pixels.flatMap((row, y) => row.map((dark, x) => dark ? <rect key={`${x}:${y}`} x={x} y={y} width="1" height="1" fill="black" /> : null))}</svg>
      <button disabled={account.busy} onClick={() => void account.cancel()}>取消登录</button>
    </div>}
    {account.message && <p className="muted" role="status">{account.message}</p>}
    {account.error && <p className="error-message" role="alert">{account.error}</p>}
    <p className="muted">登录凭据保存在 bili-dm 本机数据目录，仅用于向 B 站鉴权。会员视频仍以账号实际观看权益为准。</p>
  </section>;
}

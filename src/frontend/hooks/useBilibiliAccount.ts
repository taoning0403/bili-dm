import { useEffect, useRef, useState } from "react";
import { bilibiliService as api } from "../services/bilibiliService";
import { getErrorMessage } from "../lib/desktop";
import type { AccountStatus, LoginQr } from "../types/bilibili";
export function useBilibiliAccount() {
  const [status, setStatus] = useState<AccountStatus>({ state: "signedOut", account: null });
  const [qr, setQr] = useState<LoginQr | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const revision = useRef(0), mounted = useRef(true), running = useRef(false);
  useEffect(() => {
    mounted.current = true;
    const ticket = revision.current;
    void api.account().then(next => { if (mounted.current && revision.current === ticket) setStatus(next); }).catch(cause => { if (mounted.current) setError(getErrorMessage(cause)); });
    return () => { mounted.current = false; revision.current++; void api.cancel().catch(() => {}); };
  }, []);
  useEffect(() => {
    if (!qr) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const ticket = revision.current;
    async function poll() {
      try {
        const next = await api.poll(qr!.ticket);
        if (!active || ticket !== revision.current) return;
        if (next.state === "signedIn") { setStatus({ state: "signedIn", account: next.account }); setQr(null); setMessage("登录成功，凭据已保存到本机。"); return; }
        if (next.state === "expired") { setQr(null); setMessage("二维码已过期，请重新获取。"); return; }
        setMessage(next.state === "scanned" ? "已扫码，请在 B 站客户端确认。" : "请使用 B 站客户端扫码登录。");
        timer = setTimeout(() => void poll(), 2500);
      } catch (cause) { if (active && ticket === revision.current) { setError(getErrorMessage(cause)); setQr(null); } }
    }
    timer = setTimeout(() => void poll(), 1500);
    return () => { active = false; clearTimeout(timer); };
  }, [qr]);
  async function run(work: () => Promise<void>) {
    if (running.current) return;
    running.current = true; setBusy(true); setError("");
    try { await work(); } catch (cause) { if (mounted.current) setError(getErrorMessage(cause)); }
    finally { running.current = false; if (mounted.current) setBusy(false); }
  }
  return {
    status, qr, message, error, busy,
    start: () => run(async () => { const ticket = ++revision.current; setQr(null); const next = await api.start(); if (mounted.current && ticket === revision.current) { setQr(next); setMessage("请使用 B 站客户端扫码登录。"); } }),
    cancel: () => run(async () => { revision.current++; setQr(null); await api.cancel(); setMessage(""); }),
    verify: () => run(async () => { const next = await api.verify(); if (mounted.current) { setStatus(next); setMessage(next.state === "expired" ? "登录已失效，请重新扫码。" : "账号状态已验证。"); } }),
    logout: () => run(async () => { revision.current++; setQr(null); await api.logout(); if (mounted.current) { setStatus({ state: "signedOut", account: null }); setMessage("本地登录凭据已清除。"); } }),
  };
}

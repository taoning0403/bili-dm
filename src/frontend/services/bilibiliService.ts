import { invokeDesktop } from "../lib/desktop";
import type { AccountStatus, LoginQr, LoginPoll } from "../types/bilibili";
export const bilibiliService = {
  account: () => invokeDesktop<AccountStatus>("bilibili_account"),
  start: () => invokeDesktop<LoginQr>("bilibili_login_start", {}, 0),
  poll: (ticket: string) => invokeDesktop<LoginPoll>("bilibili_login_poll", { ticket }, 0),
  cancel: () => invokeDesktop<void>("bilibili_login_cancel"),
  verify: () => invokeDesktop<AccountStatus>("bilibili_account_verify", {}, 0),
  logout: () => invokeDesktop<void>("bilibili_logout", {}, 0),
};

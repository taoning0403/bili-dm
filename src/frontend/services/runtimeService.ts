import { invokeDesktop } from "../lib/desktop";
import type { RuntimeInfo } from "../types/runtime";

export function getRuntimeInfo(): Promise<RuntimeInfo> {
  return invokeDesktop<RuntimeInfo>("get_runtime_info");
}

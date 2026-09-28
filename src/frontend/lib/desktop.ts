import { invoke, isTauri } from "@tauri-apps/api/core";

const COMMAND_TIMEOUT_MS = 8_000;

/** Keep transport details out of pages and domain services. */
export async function invokeDesktop<T>(
  command: string,
  args?: Record<string, unknown>,
  timeoutMs = COMMAND_TIMEOUT_MS,
): Promise<T> {
  if (!isTauri()) {
    throw new Error("当前为浏览器预览。请运行 npm run desktop:dev 打开桌面应用。");
  }

  let timeoutId: ReturnType<typeof setTimeout> | undefined;

  try {
    return await Promise.race([
      invoke<T>(command, args),
      new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          reject(new Error("桌面服务响应超时，请重试或重新启动应用。"));
        }, timeoutMs);
      }),
    ]);
  } finally {
    clearTimeout(timeoutId);
  }
}

export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string" && error.length > 0) return error;
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return "无法连接桌面服务，请重试。";
}

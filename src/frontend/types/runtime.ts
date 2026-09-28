/** Wire contract for the Rust core::app_service::RuntimeInfo response. */
export interface RuntimeInfo {
  appName: string;
  appVersion: string;
  platform: string;
  architecture: string;
}

export type RuntimeState =
  | { status: "checking" }
  | { status: "ready"; info: RuntimeInfo }
  | { status: "error"; message: string };

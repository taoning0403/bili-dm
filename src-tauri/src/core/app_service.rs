use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub platform: &'static str,
    pub architecture: &'static str,
}

/// A small application service; future playback orchestration belongs in Core,
/// not in commands or UI components.
#[derive(Default)]
pub struct AppService {}

impl AppService {
    pub fn runtime_info(&self) -> RuntimeInfo {
        RuntimeInfo {
            app_name: "Bili DM",
            app_version: env!("CARGO_PKG_VERSION"),
            platform: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        }
    }
}

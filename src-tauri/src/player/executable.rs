use crate::core::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

pub fn find_mpv(resource_dir: &Path) -> AppResult<PathBuf> {
    if let Some(path) = std::env::var_os("BILI_DM_MPV_PATH") {
        let path = PathBuf::from(path);
        return if path.is_absolute() && path.is_file() {
            Ok(path)
        } else {
            Err(AppError::new(
                "MPV_NOT_FOUND",
                "BILI_DM_MPV_PATH 必须指向存在的 mpv 可执行文件（绝对路径）。",
            ))
        };
    }
    let name = if cfg!(windows) { "mpv.exe" } else { "mpv" };
    let mut candidates = vec![resource_dir.join("bin").join(name)];
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|path| path.join(name)));
    }
    if cfg!(target_os = "macos") {
        candidates.extend([
            PathBuf::from("/opt/homebrew/bin/mpv"),
            PathBuf::from("/usr/local/bin/mpv"),
        ]);
    } else if cfg!(target_os = "linux") {
        candidates.extend([
            PathBuf::from("/usr/bin/mpv"),
            PathBuf::from("/usr/local/bin/mpv"),
        ]);
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            AppError::new(
                "MPV_NOT_FOUND",
                "未找到 mpv。请按 README 安装播放器依赖后重试。",
            )
        })
}

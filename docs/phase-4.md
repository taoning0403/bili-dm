# Phase 4：mpv 与按需流式播放

架构：PlaybackService 编排 TorrentEngine、私有 MediaServer 和 PlayerBackend；mpv 进程通过 Unix socket / Windows named pipe 的 JSON IPC 控制。HTTP 仅绑定 127.0.0.1 随机端口，以随机 token 暴露当前选中文件的字节范围，不暴露管理 API。视频字节不经过 Tauri JSON IPC。

实现：本地文件校验与系统文件选择器、磁力选片后下载、暂停/音量/绝对 seek/停止、实时播放与下载状态、退出清理。切换媒体或停止会暂停旧 torrent；视频源凭据随切换失效。

测试：7 个 Rust 单元测试 + 1 个真实 loopback HTTP 测试通过；cargo check 与 Clippy 通过。本机 mpv 0.40.0 / FFmpeg 7.1.1。自行生成的 30 秒本地测试视频通过播放、暂停、25% 音量、15 秒 seek、继续播放及停止。用户第二个磁力也全部通过：初次播放 position=2.461s，downloaded=33725628/626695356（约 5.4%）；中段 seek 后 position=712.086s，downloaded=80387260/626695356（约 12.8%），证明未等待完整下载。

复现：cargo run --manifest-path src-tauri/Cargo.toml --example playback_smoke -- <本地视频或磁力链接> [文件index]。该命令使用临时缓存，完成后停止 mpv、torrent 并清理测试下载。

已知问题：本版视频使用独立 mpv 窗口，尚未嵌入主窗口；mpv 是外部运行依赖，尚未随安装包分发。macOS 已实测，Windows/Linux 未实测。字幕仅识别，尚未提供挂载操作。主界面播放控制在 Phase 5 接入。

修改文件：

- `docs/phase-4.md`
- `src-tauri/Cargo.lock`
- `src-tauri/Cargo.toml`
- `src-tauri/examples/playback_smoke.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/player.rs`
- `src-tauri/src/core/app_service.rs`
- `src-tauri/src/core/mod.rs`
- `src-tauri/src/core/playback_service.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/media/mod.rs`
- `src-tauri/src/media/range.rs`
- `src-tauri/src/media/stream_server.rs`
- `src-tauri/src/player/executable.rs`
- `src-tauri/src/player/ipc.rs`
- `src-tauri/src/player/mod.rs`
- `src-tauri/src/player/mpv.rs`
- `src-tauri/src/torrent/mod.rs`
- `src-tauri/src/torrent/models.rs`
- `src-tauri/src/torrent/rqbit.rs`
- `src-tauri/tests/media_transport.rs`

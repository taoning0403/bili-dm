# Phase 6：SQLite 和 0.2.0 交付

架构：LibraryRepository 是 Core 依赖的本地仓储接口，SqliteLibrary 封装 bundled SQLite、连接互斥、spawn_blocking、WAL、外键和事务。001_library.sql 创建任务/媒体表，以 user_version = 1 管理迁移；更新版本 schema 会被拒绝。Tauri 启动层注入数据库路径。

行为：成功解析时保存任务及目录快照；播放时保存媒体身份，读取到有效时长后更新一次；停止/切换时更新任务状态。快照与切换串行，避免把旧视频时长写到新文件。重启将 downloading 状态改为 paused，历史读取不启动网络。UI 提供最近 20 个磁力的重新加载入口。

测试方法：`npm run check`、`npm run rust:test`、`npm run desktop:build -- --bundles app`；真实播放回归可使用 playback_smoke 示例。SQLite 集成测试覆盖重开保留记录、相同媒体不重复插入、未知时长不覆盖已知值、状态不自动恢复下载、外键失败回滚、非法时长及新 schema 拒绝。

实测：11 项 Rust 测试通过（7 单元 + 3 SQLite + 1 HTTP Range），TypeScript strict、rustfmt、Clippy -D warnings 和生产构建通过。0.2.0 macOS arm64 .app 为 25.38 MiB。引入数据库后，本地 30 秒视频的播放/暂停/音量/中段 seek/继续/停止 smoke 再次通过。

原生 0.2.0 实测第二个用户磁力：目录和历史记录可见；播放后 SQLite 的任务状态为 downloading，媒体 path 为 torrent://e07ed7410558567358b92e455a6a226e68a96a09/0，时长为 1422.123 秒。界面进度滑块跳至 11:51，继续播放后进度到达 11:55；mpv IPC 同时确认实际位置约 711 秒，排除只改变 UI 的情况。停止后数据库状态变成 paused。测试下载及自建视频样本已清理，保留任务历史供使用。

重启验收：重新打开打包应用，仍显示 1 条最近磁力；展开后显示正确文件名、文件数和“已停止下载”。播放器显示“尚未播放”，缓存目录为空，历史不会自动触发下载。

已知问题：仅 macOS 实测；mpv 为外部依赖和独立窗口。历史不保存引擎 metadata 对象，重启后重新播放磁力仍需解析；暂无缓存清理 UI 或本地媒体历史页面。字幕仅分类，不支持挂载。后续 metadata/弹幕/匹配关系通过媒体 ID 的关联表扩展，不创建空插件实现。

修改文件：

- `README.md`、`docs/architecture.md`、`docs/phase-6.md`
- `package.json`、`package-lock.json`
- `src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`
- `src-tauri/examples/playback_smoke.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/mod.rs`、`src-tauri/src/commands/library.rs`
- `src-tauri/src/core/app_service.rs`、`src-tauri/src/core/playback_service.rs`
- `src-tauri/src/database/mod.rs`、`src-tauri/src/database/models.rs`、`src-tauri/src/database/sqlite.rs`
- `src-tauri/src/database/migrations/001_library.sql`
- `src-tauri/tests/library.rs`
- `src/frontend/components/MagnetInput.tsx`、`src/frontend/components/RecentTorrents.tsx`
- `src/frontend/hooks/usePlayback.ts`、`src/frontend/hooks/useRecentTorrents.ts`
- `src/frontend/services/libraryService.ts`、`src/frontend/types/library.ts`
- `src/frontend/pages/HomePage.tsx`、`src/frontend/styles.css`

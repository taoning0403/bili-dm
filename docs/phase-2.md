# Phase 2：磁力元数据

实现：librqbit 9.0.1 适配器、TorrentEngine 接口、只读目录加载、取消、120 秒后端超时和前端错误提示。未选片时不启动内容下载。

架构：MagnetInput → useTorrent → torrentService → Tauri commands → AppService → TorrentEngine。元数据和 engine 对象不暴露给 UI。退出时停止 session。

测试：TypeScript strict、cargo check --all-targets、cargo test --lib、Clippy -D warnings。真实 magnet probe 使用 src-tauri/examples/inspect_magnet.rs；用户第一个 hash b289ee… 解析出 12 个 MKV，第二个 e07ed7… 解析出单个 626695356 字节 MKV。3 个磁力输入测试覆盖非法 hash、空链接和移除不可信 so 文件选择范围。

已知问题：公网解析依赖在线节点；纯 BT v2 暂不支持。当前阶段不播放。Windows/Linux 未实测。

修改文件：

- `README.md`
- `docs/phase-2.md`
- `src-tauri/Cargo.lock`
- `src-tauri/Cargo.toml`
- `src-tauri/examples/inspect_magnet.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/torrent.rs`
- `src-tauri/src/core/app_service.rs`
- `src-tauri/src/core/error.rs`
- `src-tauri/src/core/mod.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/torrent/mod.rs`
- `src-tauri/src/torrent/models.rs`
- `src-tauri/src/torrent/rqbit.rs`
- `src/frontend/components/AppHeader.tsx`
- `src/frontend/components/MagnetInput.tsx`
- `src/frontend/hooks/useTorrent.ts`
- `src/frontend/lib/desktop.ts`
- `src/frontend/lib/format.ts`
- `src/frontend/pages/HomePage.tsx`
- `src/frontend/services/torrentService.ts`
- `src/frontend/styles.css`
- `src/frontend/types/torrent.ts`

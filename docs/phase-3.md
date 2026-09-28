# Phase 3：媒体识别与选择

架构：media/files 是独立于引擎和 UI 的分类/选择策略；torrent 使用通用 MediaFile 描述，前端目录树保留 engine 文件 index。支持视频 mp4/mkv/avi/webm/mov/m4v/ts/m2ts 与字幕 srt/ass/ssa/vtt/sub/idx 分类。默认选择最大的非空视频，用户可改选；同大小保留第一个。

测试：5 个 Rust 测试通过；TypeScript strict 与 Clippy 通过。新增测试覆盖大小写、伪装扩展名、最大文件、稳定 index、同大小、空文件和空列表。

已知问题：视频扩展名识别不等于解码成功，最终由 mpv 验证。字幕仅分类；挂载尚未接入。本阶段不播放。

修改文件：

- `docs/phase-3.md`
- `src-tauri/src/media/files.rs`
- `src-tauri/src/media/mod.rs`
- `src-tauri/src/torrent/models.rs`
- `src-tauri/src/torrent/rqbit.rs`
- `src/frontend/components/FileCatalog.tsx`
- `src/frontend/hooks/useTorrent.ts`
- `src/frontend/pages/HomePage.tsx`
- `src/frontend/styles.css`
- `src/frontend/types/torrent.ts`

# 状态边界

Phase 1 的运行状态只属于启动页，使用 `hooks/useRuntimeInfo.ts` 管理。
跨页面的播放会话和 torrent 任务状态在实际接入时放入此目录；当前不引入全局状态库。
Store 通过 `services/` 调用命令，不直接引用 Tauri、torrent engine 或 mpv。

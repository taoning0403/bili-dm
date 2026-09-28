# Phase 5：播放器界面

修改文件：`src/frontend/components/{AppHeader,FileCatalog,PlayerControls}.tsx`、`src/frontend/hooks/usePlayback.ts`、`src/frontend/services/playerService.ts`、`src/frontend/types/player.ts`、`src/frontend/lib/{desktop,format}.ts`、`src/frontend/pages/HomePage.tsx`、`src/frontend/styles.css`、本报告。

架构：页面组合 useTorrent/usePlayback；hooks 调用 services；services 通过 Tauri commands 访问 Rust Core。PlayerControls 只接收状态和控制回调，不知道 torrent 实现。轮询不重叠，操作期间抛弃旧轮询结果，重复点击被阻止。目录按文件名自然排序，仍使用引擎真实 index 选片。

测试方法：`npm run typecheck`、`npm run build`、`npm run desktop:build -- --bundles app`。打开原生应用，使用“打开本地视频”选择视频，测试暂停、进度和音量；粘贴磁力，加载目录，手动选择非默认视频并播放。点击停止应清空当前媒体和下载状态。

实测：macOS 原生应用的系统文件选择器可打开生成的 30 秒视频，界面显示播放时间和总时长，暂停按钮变成继续播放。第一个磁力显示 12 个视频，手动选第 1 集后，播放标题与下载状态均对应该文件。Rust 端完整回归还验证了暂停、音量、seek 和停止：第一个磁力在下载约 3.9% 时开始播放，中段 seek 后下载约 9.2%；第二个磁力数据见 Phase 4。

已知问题：视频在独立 mpv 窗口，主界面负责控制；字幕文件只显示类型，尚不能挂载。元数据获取与缓冲速度依赖在线节点；浏览器预览不提供原生能力。Windows/Linux 原生 UI 尚未实测。

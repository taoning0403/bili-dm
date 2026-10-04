# Frame Player 参考重实现记录

日期：2026-10-04。Bili DM 从 b76ea82（0.2.0）升级到 0.3.0，工作分支 codex/frame-playback。

## 参考基线与范围

检查的 Frame Player 源码为 [e9767259d5f5ca25a5b2a5951b1ebc80c702ea73](https://github.com/risenxxx/frame-player/tree/e9767259d5f5ca25a5b2a5951b1ebc80c702ea73)，版本 1.24.3。参考的是元数据解析、按文件选择、流读取优先级、缓冲区间、播放队列、续播和原生 libmpv 的组织方式。

关键参考入口：[torrent.rs](https://github.com/risenxxx/frame-player/blob/e9767259d5f5ca25a5b2a5951b1ebc80c702ea73/src-tauri/src/torrent.rs)、[playlist](https://github.com/risenxxx/frame-player/blob/e9767259d5f5ca25a5b2a5951b1ebc80c702ea73/src/lib/playlist.svelte.ts)、[resume](https://github.com/risenxxx/frame-player/blob/e9767259d5f5ca25a5b2a5951b1ebc80c702ea73/src/lib/resume.svelte.ts)、[player](https://github.com/risenxxx/frame-player/blob/e9767259d5f5ca25a5b2a5951b1ebc80c702ea73/src/lib/player.svelte.ts)。应用实现继续使用本仓库 React/Tauri/Rust 分层，没有引入 Frame 的 Svelte 应用代码或应用插件。

复用了其单独构建的 LGPL 原生运行库及 macOS NSView 补丁；固定归档、校验值、各库的源码和许可证见 [第三方说明](../THIRD-PARTY-NOTICES.md)。mpv 配置依据 [官方手册](https://mpv.io/manual/stable/)，尤其是 video-margin-ratio-* 与 sub-use-margins 的关联。

## 交付模块

| 模块 | 改动 |
| --- | --- |
| torrent/rqbit.rs、buffer.rs | 延迟创建 Session；元数据缓存；磁力取消/超时；种子导入；当前视频及字幕选择；一个后续文件预取；真实分片范围 |
| media/playlist.rs、stream_server.rs | 自然排序、字幕匹配、Range 传输及已建立响应的取消 |
| player/native.rs、mpv.rs | 动态加载 libmpv C API、内嵌画面、轨道/章节/倍速/逐帧/延迟；删除旧外部进程和 IPC 模块 |
| core/playback_service.rs | 后台播放状态机、快照、取消、队列、EOF、续播、自动字幕、偏好保存及退出清理 |
| database/migrations/002_playback.sql | 保留旧媒体和历史，新增播放进度与偏好 |
| commands、lib.rs | 文件选择器、队列/字幕/窗口命令、原生句柄和异步退出 |
| React 播放页面 | 主窗口画面、资源/队列侧栏、缓存进度、轨道设置、快捷键及系统全屏状态同步 |
| scripts/setup-player.mjs、Tauri bundle | 固定 SHA-256 安装运行库，随 app 携带动态库及许可文本，macOS 本地 ad-hoc 签名 |

## 验证结果

- npm run check：TypeScript strict、rustfmt 和 Clippy -D warnings 通过。
- Rust 自动化测试 21 个通过：10 个单元测试、4 个 SQLite 集成测试、2 个真实 HTTP 测试、5 个播放状态机测试。
- 迁移测试先填充 schema 1 历史和媒体，再验证 schema 2 保留数据、暂停原下载状态并保存新进度。
- 阻塞流测试验证 source 撤销会结束未完整的 HTTP body，并销毁始终等待缺失分片的 reader。
- 真实 libmpv 本地文件 smoke：开始播放、暂停、音量 25%、中段 seek、继续、停止通过。

受控真实 BT + libmpv smoke 使用两个 40 秒、约 16.6 MiB 的合成视频和同名 SRT，本机 tracker + 限速 1.5 MB/s seeder：

```text
PASS first frame at 17.0% downloaded; queue=2 files
PASS seek, speed and verified piece map
PASS stop and per-file resume at 21.00s
PASS one-ahead prefetch starts only after current file completes
PASS automatic companion subtitles and EOF next episode
```

原生 macOS Debug 应用使用独立 BILI_DM_TEST_ROOT 验证，未把受控种子写入用户的正式数据库：

- 从磁力读取 3 个文件的目录，选择视频后显示主窗口内的真实视频画面。
- 下载中播放、暂停、手动切集和 EOF 自动转到第二集通过。
- 关闭后重启不自动播放，历史任务为 paused；点击前进程无网络 socket。
- 重启主动打开后恢复到上次 15 秒、28 秒位置，元数据/视频缓存复用。
- .torrent 系统对话框导入、本地视频对话框播放通过。
- 外挂 SRT 自动选中，中文和英文文字在普通窗口及全屏中实际可见。
- 播放器全屏按钮、系统全屏按钮和 Escape 的布局同步通过。
- 停止撤销媒体与队列；播放中关闭窗口正常结束进程，无外部 mpv 进程残留。

正式 Release 应用版本 0.3.0，路径 src-tauri/target/release/bundle/macos/Bili DM.app，约 74 MiB。包含 34 个动态库、25 份原生许可文本及第三方说明；动态库依赖检查未发现 Homebrew 或临时构建目录依赖。codesign --verify --deep --strict 通过。

最终签名 Release 包另行实测了本地视频画面、暂停、从 24.5 秒跳回 14.5 秒和正常退出。验证前备份本地库，结束后仅删除本次生成视频对应的临时媒体/进度记录，并逐表核对既有记录与备份一致。

测试 seeder、测试应用进程、合成视频/缓存、参考仓库临时副本、下载归档和 Debug 测试包均已清理，保留正式 Release 包及开发所需的播放器运行库。

签名复测发现并修复了两个原生发布问题：不同签名身份的 LGPL 库需要本应用的 library-validation entitlement；mpv 默认 Lua 脚本会使 hardened runtime 因未签名执行内存终止进程。当前禁用这些未使用脚本，保留 hardened runtime，无需 unsigned-executable-memory 权限。原生交互、签名后的实际播放、headless 解码和公网可用性分别验证。

## 明确边界

本次没有使用公网 swarm 复验；公网可用性和首帧等待仍受节点、网络和容器结构影响。0.2.0 的历史公网磁力结果不能直接当作本版本证据。

只完成 macOS arm64 原生运行与打包。Windows/Linux 适配、Wayland、HDR、特定硬件解码格式、完整 ASS/VobSub 样本矩阵尚未验证。本地 ad-hoc 签名不等于 Developer ID 签名或公证，尚未做对外发布。

Frame 的资源目录、在线字幕服务、投屏、一起看、帧预览和 HDR 专项调校没有移植；B 站匹配、弹幕融合和插件运行时保持独立后续范围。当前分片栏是按字节比例估算时间，未实现容器索引到分片的精确映射。

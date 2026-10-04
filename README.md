# Bili DM

本地优先的桌面媒体播放器，当前版本 **0.3.0**。磁力播放链路参考 [Frame Player](https://github.com/risenxxx/frame-player/tree/e9767259d5f5ca25a5b2a5951b1ebc80c702ea73) 重新实现：Tauri 2 + React + Rust，librqbit 负责 BT，内嵌 libmpv 负责解码和显示，SQLite 保存历史与续播进度。

视频直接显示在应用主窗口。支持磁力链接、种子文件、本地视频、自然排序播放队列、自动连播、下一集预取、断点续播、外挂字幕、音轨切换、倍速、章节、逐帧及全屏。播放器运行库随 macOS 应用包携带，不再启动外部 mpv 进程。

新增内置 **Bilibili 弹幕混合**：批量解析 BV / 视频链接及分 P，以画面和时长匹配当前视频，支持手动调整多个截取区间、混合多套弹幕、保存/导出 XML，并在播放器中独立于字幕显示。入口为右侧“混合弹幕”，先打开视频再配置。详见 [使用与验证说明](docs/danmaku-mixer.md)。

## 环境与启动

已验证平台：macOS Apple Silicon。需要 Node.js 22.12+、npm、Rust stable、Xcode Command Line Tools。SQLite 随 Rust 构建，libmpv 通过固定版本和 SHA-256 校验的脚本准备：

```sh
npm ci
source "$HOME/.cargo/env"   # cargo 已在 PATH 时可省略
npm run player:setup
npm run desktop:dev
```

运行库下载到 src-tauri/lib，约 48 MiB，已从 Git 排除。macOS 使用支持 NSView 嵌入的 mpv 0.41.0；普通 Homebrew mpv 不能直接代替这一构建。运行库来源、构建补丁及许可证见 [第三方说明](THIRD-PARTY-NOTICES.md)。

Windows/Linux 尚未完成原生验证，也没有自动运行库安装脚本。适配器预留 Win32/X11 窗口句柄；如自行移植，须在 src-tauri/lib 放置 libmpv-2.dll 或 libmpv.so.2 及其依赖，并验证透明 WebView、窗口叠放和打包。Wayland 嵌入暂不支持。

仅运行 npm run dev 是浏览器预览，无法验证 Tauri 命令、BT 引擎或原生画面。

## 使用方法

1. 粘贴 magnet 并加载目录，或点击“打开种子”选择 .torrent。解析只取得元数据，不下载整季正文；磁力解析可取消，120 秒超时。
2. 视频按文件名自然排序，默认选第一集，保留引擎的真实文件 index。点击“播放选中视频”后只选择当前视频及匹配字幕。
3. 视频通过私有 localhost HTTP Range 流送入 libmpv。拖动进度时，rqbit 的读取优先级跟随所请求的字节范围，无需下载完整文件。
4. 播放队列支持切集、自动连播、单集循环和列表循环。当前视频完整下载后才预取一个后续视频，可随时关闭预取。
5. 相同文件名或带语言后缀的字幕自动下载并挂载；单视频种子会匹配其字幕文件。也可在“音轨 / 字幕”中手动加载本地字幕、选轨或调整延迟。
6. 续播位置每 5 秒以及控制、切集、停止、退出时保存。已播完、开头不足 3 秒、距离结尾不足 5 秒的文件重新从头播放。
7. 停止会撤销媒体 URL、取消等待读取并暂停下载。重启只显示历史，须主动选择播放才会连接 BT 网络。

进度条浅色区域表示已校验分片，按文件字节比例估算时间位置；容器的可变码率会使该估算与真实时间有所偏差。libmpv 的“缓冲秒数”另行显示。

| 操作 | 快捷键 |
| --- | --- |
| 播放 / 暂停 | Space / K，单击画面 |
| 前后跳转 10 秒 / 1 秒 | ← → / Shift + ← → |
| 音量 / 静音 | ↑ ↓ / M |
| 上一集 / 下一集 | PageUp / PageDown |
| 前后逐帧 | , / . |
| 全屏 / 退出 | F、双击画面 / Escape |

视频候选：mp4、mkv、avi、webm、mov、m4v、ts、m2ts。字幕：srt、ass、ssa、vtt、sub、idx；自动外挂字幕限制为每个 20 MiB。候选扩展名不保证实际内容能够解码。

## 数据与升级

- SQLite：app_data_dir()/library.sqlite3；macOS 为 ~/Library/Application Support/dev.bilidm.player/library.sqlite3。
- 视频和字幕缓存：app_cache_dir()/torrents/<info-hash>/。
- 元数据缓存：app_cache_dir()/torrents/metadata/<info-hash>.torrent；再次打开时可从本地恢复目录。
- schema 2 自动从 schema 1 事务迁移，保留原有 torrent_tasks/media，增加播放进度和播放偏好。拒绝打开更新版本的 schema。

任务历史、媒体缓存和续播位置保存在本机。启动不创建 rqbit 网络会话，遗留 downloading 状态恢复为 paused。缓存不会自动删除；暂未提供缓存管理页面，清理前请退出应用。调试构建可设置 BILI_DM_TEST_ROOT 将数据库和缓存隔离到测试目录；正式构建忽略此变量。

## 检查与打包

```sh
npm run check          # TypeScript、rustfmt、Clippy -D warnings
npm run rust:test      # Rust 单元/集成测试
npm test              # 弹幕时钟与排布测试
npm run desktop:build -- --bundles app
```

macOS 产物：src-tauri/target/release/bundle/macos/Bili DM.app，包含播放器库和许可证。默认使用本地 ad-hoc 签名；尚未配置 Developer ID 分发签名、公证或自动更新。

Entitlements.plist 允许本应用在 hardened runtime 下加载第三方 libmpv 动态库，避免签名后无法开始播放。不会修改系统安全设置。

测试覆盖 metadata 参数、自然排序、字幕匹配、分片范围、HTTP Range/撤销阻塞读取、旧库迁移、续播、取消打开、解码错误清理、自动连播与预取边界。

真实引擎 smoke：

```sh
cargo run --manifest-path src-tauri/Cargo.toml --example inspect_magnet -- 'magnet:?...'
cargo run --manifest-path src-tauri/Cargo.toml --example playback_smoke -- /absolute/path/video.mp4
cargo run --manifest-path src-tauri/Cargo.toml --example playback_smoke -- 'magnet:?...' 0
cargo run --manifest-path src-tauri/Cargo.toml --example streaming_smoke -- /absolute/path/fixtures
```

streaming_smoke 的输入目录需要两个可 seek、时长大于 30 秒的视频（例如 E01.mp4、E02.mp4）及同名 E01.zh.srt。例子创建临时种子、本机 tracker 和限速 seeder，验证首帧发生在下载完成前、跳转、续播、字幕选中、当前集下载完成后预取下一集，以及 EOF 自动切集。追加 hold-seeder 参数可保留受控源进行 GUI 检查，Ctrl+C 正常释放。

本次受控样本约 16.6 MiB / 集，在约 17% 下载进度时开始播放；这是本机环回 swarm 的结果，不能代表公网磁力可用性或首帧速度。0.3.0 的原生 macOS 验证及参考范围见 [重实现记录](docs/frame-player-reimplementation.md)。

## 架构与范围

保持 Frontend → Service → Tauri Command → Rust Core → TorrentEngine / PlayerBackend / LibraryRepository。视频字节经 rqbit reader → 127.0.0.1 随机端口及 token → libmpv，不经过 Tauri JSON。libmpv 原生画面位于透明 WebView 下方，React 控制面板通过视口比例限定视频区域。

详见 [架构说明](docs/architecture.md)。Frame Player 的 TMDB/Torznab 目录、投屏、一起看、在线字幕搜索、帧预览、HDR 专项调校等未移植。B 站弹幕解析和视觉匹配由独立内置模块提供；账户、AI 和第三方插件运行时尚未实现。

纯 BT v2 magnet 暂不支持；无节点或数据损坏时可能无法播放。硬件解码使用 mpv auto-safe，由实际编解码器和设备决定；Windows/Linux、公网 swarm、HDR 和各种字幕格式仍需目标环境验证。

Phase 1–6 文档保留为 0.2.0 历史记录：[初始化](docs/phase-1.md)、[磁力](docs/phase-2.md)、[分类](docs/phase-3.md)、[旧播放链路](docs/phase-4.md)、[旧 UI](docs/phase-5.md)、[持久化](docs/phase-6.md)。当前行为以本文件和架构说明为准。

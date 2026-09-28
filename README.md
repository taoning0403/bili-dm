# Bili DM

本地优先的桌面媒体播放器，当前版本 **0.2.0**。使用 Tauri 2、React、TypeScript strict、Vite、Rust；librqbit 负责 torrent，mpv 负责播放，SQLite 保存任务和媒体。

可粘贴磁力链接、读取文件目录、选择视频边下载边播放，也可通过系统文件选择器打开本地视频。主窗口支持暂停、继续、前后 10 秒、拖动进度、音量、下载速度和进度。**视频显示在独立 mpv 窗口，尚未嵌入主窗口。**

不需要账号，没有云服务、业务后端 API、AI、弹幕或自动匹配。字幕文件已分类，外挂字幕挂载尚未实现。插件目录只有扩展约定。

## 环境与启动

需要 Node.js 22.12+、npm、Rust stable、Tauri 平台依赖和 mpv。SQLite 通过 bundled rusqlite 编译，无需数据库服务。本机验证环境：macOS arm64、Rust 1.98.1、mpv 0.40.0；Windows/Linux 尚未原生实测。

- macOS：Xcode Command Line Tools；mpv 可用 `brew install --formula mpv` 安装。
- Windows：Microsoft C++ Build Tools 的桌面 C++ 工具、WebView2、Rust MSVC 工具链；安装 mpv，将 mpv.exe 所在目录加入 PATH。
- Linux：安装发行版的 GTK/WebKitGTK 开发依赖和 mpv。Ubuntu/Debian 示例：

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev mpv
  ```

平台准备详见 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/) 和 [mpv 安装说明](https://mpv.io/installation/)。

```sh
npm ci
# macOS/Linux：如果 cargo 不在 PATH，先执行
source "$HOME/.cargo/env"
npm run desktop:dev
```

Tauri 自动启动本地 Vite 并打开原生窗口，首次 Rust 编译较慢。终端按 Ctrl+C 结束开发任务。仅运行 `npm run dev` 是浏览器预览，无法调用原生播放器能力。

mpv 搜索顺序：环境变量 `BILI_DM_MPV_PATH` 指定的绝对路径 → 应用资源目录 bin/mpv（Windows 为 mpv.exe）→ PATH → macOS Homebrew / Linux 常见路径。当前安装包不内置 mpv，缺失时界面提示安装。例如：

```sh
BILI_DM_MPV_PATH=/opt/homebrew/bin/mpv npm run desktop:dev
```

## 使用方法

1. 粘贴完整 `magnet:?xt=urn:btih:...`，点击“加载目录”。只读取 metadata，不下载正文。支持取消；120 秒没有取得目录时返回可重试错误。
2. 目录显示路径、大小和视频/字幕/其他类型，默认选最大非空视频。可手动改选后点击“播放选中视频”。自然排序不改变引擎文件 index。
3. 视频在 mpv 窗口播放，主界面显示缓冲、时长和下载状态。随机拖动会向引擎请求对应字节范围，不等待完整下载。
4. “停止”停止播放并暂停当前下载；切换视频释放旧数据源。关闭 mpv 窗口后下一次状态轮询会停止下载。退出主应用关闭 mpv 和 torrent 会话。
5. “打开本地视频”使用系统文件选择器，无需磁力即可播放本地文件。
6. “最近加载的磁力”保留最近 20 条任务入口；点击重新解析。重启只读取本地历史，不自动下载或播放。

视频候选：mp4、mkv、avi、webm、mov、m4v、ts、m2ts，扩展名不区分大小写。候选识别不保证文件可解码，实际播放取决于内容和 mpv。

外部字幕识别：srt、ass、ssa、vtt、sub、idx。本版没有“磁力/本地视频 + 磁力/本地字幕”挂载操作；容器内自带字幕由 mpv 自身处理。

## 本地数据

操作系统目录由 Tauri 注入，没有写死个人路径：

- SQLite：`app_data_dir()/library.sqlite3`，macOS 为 `~/Library/Application Support/dev.bilidm.player/library.sqlite3`。
- 正文缓存：`app_cache_dir()/torrents/<info-hash>/`，macOS 位于 `~/Library/Caches/dev.bilidm.player/torrents/`。
- mpv IPC：Unix 临时私有目录内的 socket，Windows 随机 named pipe；退出释放。

迁移使用 PRAGMA user_version，当前 schema 为 1。torrent_tasks 保存 torrent_id、magnet_uri、created_at、status，以及名称、目录快照、更新时间；media 保存 id、path、filename、duration，以及可选 torrent/file 引用。时长在 mpv 读取成功后保存。解析失败的链接不保存成任务。

视频缓存保留用于重新播放时校验复用，暂无自动清理或管理页面。**任务历史不等于离线 metadata 缓存**：重启后播放磁力仍需重新解析节点。清理缓存前退出应用；数据库和正文缓存可独立管理。

## 目录与架构

```text
src/frontend/
  components/       输入、目录、控制面板、历史等展示组件
  pages/            页面组合
  hooks/            异步状态、串行轮询、操作互斥
  services/         用例与命令调用
  types/            IPC DTO
  lib/              IPC、错误和格式化
  stores/           跨页面状态预留
src-tauri/src/
  commands/         Tauri 参数和系统对话框适配
  core/             AppService / PlaybackService 编排
  torrent/          TorrentEngine 接口、librqbit 适配
  media/            文件分类、单范围 HTTP 字节流
  player/           PlayerBackend 接口、mpv IPC 适配
  database/         LibraryRepository、SQLite、版本化迁移
  lib.rs            依赖装配、生命周期
  main.rs           启动错误处理
plugins/README.md   扩展约定，没有插件运行时
```

调用方向：**Frontend → Service → Tauri Command → Rust Core → 接口适配器**。视频字节走 librqbit seekable reader → 私有 loopback Range 流 → mpv，不经过 JSON IPC。媒体传输只监听 127.0.0.1 随机端口，随机 token 限定选中文件，不提供业务管理 API 或任意路径访问。详见 [架构说明](docs/architecture.md)。

## 检查与构建

```sh
npm run check       # TS strict、rustfmt、Clippy -D warnings
npm run rust:test   # 7 个单元测试、SQLite 3 个集成测试、HTTP Range 集成测试
npm run build       # 前端生产构建
npm run desktop:build
# macOS 仅生成 .app（本次交付使用）
npm run desktop:build -- --bundles app
```

产物在 src-tauri/target/release/bundle/，各平台应在目标系统构建验证。尚未配置发布签名、公证或自动更新。提交了 npm/Cargo lock；Clippy 禁止 unwrap() / expect()。

真实引擎与播放器验证命令（需要 mpv，网络结果取决于节点）：

```sh
cargo run --manifest-path src-tauri/Cargo.toml --example inspect_magnet -- 'magnet:?...'
cargo run --manifest-path src-tauri/Cargo.toml --example playback_smoke -- 'magnet:?...' 10
cargo run --manifest-path src-tauri/Cargo.toml --example playback_smoke -- /absolute/path/video.mp4
```

inspect_magnet 只取 metadata。playback_smoke 验证播放、暂停、25% 音量、跳到中段、继续和停止；最后一个数字是可选的引擎文件 index。例子使用临时缓存，退出后停止会话并清理。本次用户提供的两个磁力都实测通过：

| Info hash 前缀 | 目录 | 流式验证 |
| --- | --- | --- |
| b289ee90 | 12 个 MKV；选第 1 集（index 10） | 下载约 3.9% 开始播放；中段 seek 后约 9.2% |
| e07ed741 | 1 个 MKV | 下载约 5.4% 开始播放；中段 seek 后约 12.8% |

GUI 验证覆盖系统文件选择、暂停、磁力目录加载、改选和真实播放。这是 macOS 本机当次结果，不保证所有磁力可用或固定首帧等待时间。

## 分阶段记录

| 阶段 | 记录 |
| --- | --- |
| Phase 1 | [工程初始化和 IPC](docs/phase-1.md) |
| Phase 2 | [magnet 和 metadata](docs/phase-2.md) |
| Phase 3 | [视频与字幕分类](docs/phase-3.md) |
| Phase 4 | [mpv 与流式播放](docs/phase-4.md) |
| Phase 5 | [目录选片和播放器 UI](docs/phase-5.md) |
| Phase 6 | [SQLite 持久化](docs/phase-6.md) |

每阶段独立提交，报告包括修改文件、架构、测试方法和已知问题。

## 当前限制与故障处理

- 支持 BT v1 / hybrid btih magnet，纯 BT v2 暂不支持。无节点时可取消或等待超时重试。
- 独立 mpv 窗口未嵌入 Tauri，mpv 尚未随安装包分发。Windows/Linux 的窗口、named pipe、依赖和打包需要验证。
- 缓冲慢时查看速度和连接数；可停止后重新选择。IPC 超时只结束前端等待，不取消已提交命令；metadata 另有取消入口。
- 一次播放一个视频；本地路径需 UTF-8。没有断点续播位置、缓存清理 UI 或完整任务队列。
- 字幕挂载、弹幕、metadata、外部 match.json 读取和插件运行时为后续工作。

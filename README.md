# Bili DM

本地优先的跨平台桌面媒体播放器。使用 Tauri 2、React、TypeScript、Vite 与 Rust。
后续以 torrent engine 提供流式数据、mpv/libmpv 播放、SQLite 保存本地任务和媒体。

**当前完成 Phase 2：可从磁力链接读取文件目录；下载与播放将在后续阶段接入。**
应用不需要账号，不包含云端服务、业务后端 API、AI、弹幕或自动匹配功能。

## 环境准备

- Node.js 22.12+，npm；提交 `package-lock.json`，后续安装使用 `npm ci`。
- Rust stable（本次使用 1.98.1），通过 rustup 安装；`rust-toolchain.toml` 声明 rustfmt、clippy。
- macOS：Xcode Command Line Tools，可运行 `xcode-select --install` 安装。
- Windows：Microsoft C++ Build Tools 的“使用 C++ 的桌面开发”、WebView2，以及对应的 Rust MSVC 工具链。
- Linux：GTK/WebKitGTK 等系统依赖。Ubuntu/Debian 示例：

  ```sh
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

各发行版及 Windows 安装详情见 [Tauri 官方环境准备](https://v2.tauri.app/start/prerequisites/)。
本阶段无需安装 torrent engine、mpv 或 SQLite。

若 Rust 已安装但终端提示找不到 `cargo`，macOS/Linux 当前会话执行：

```sh
source "$HOME/.cargo/env"
```

本次本机 rustup 使用 `--no-modify-path` 安装，没有改动 Shell 配置；新终端也需要上面的命令，或自行将 Cargo bin 目录加入 PATH。

## 启动

在项目根目录运行：

```sh
npm ci
npm run desktop:dev
```

Tauri 会自动启动绑定 `127.0.0.1:1420` 的 Vite 开发服务、编译 Rust，并打开原生窗口。
首次 Rust 构建需要下载和编译依赖，耗时明显长于后续构建。
开发服务仅用于本地前端开发；打包后使用内嵌静态资源，不运行业务 HTTP 服务。

窗口中的“运行状态”通过真实 IPC 命令 `get_runtime_info` 显示 Rust 返回的版本、系统和架构。
点击“重新检查”可重复验证调用。页面没有模拟成功数据。
关闭终端任务使用 `Ctrl+C`。

只预览前端时可运行 `npm run dev` 并打开 `http://127.0.0.1:1420`。
浏览器无法访问 Tauri IPC，会明确提示启动桌面应用，这不代表 Rust 已通过验证。
不要同时启动独立 Vite 和 `desktop:dev`；端口被占用时先停止对应任务。

## 检查与构建

```sh
# TypeScript strict、Rust 格式、Clippy（警告视为错误）
npm run check

# 前端类型检查及生产资源构建
npm run build

# 构建当前操作系统的桌面安装包
npm run desktop:build

# 仅构建桌面二进制，用于本地验证，不生成安装包
npm run tauri -- build --no-bundle
```

也可单独运行 `npm run typecheck`、`npm run rust:fmt`、`npm run rust:clippy`。
修复 Rust 格式使用 `cargo fmt --manifest-path src-tauri/Cargo.toml --all`。
`Cargo.lock` 必须提交，Clippy 使用 `--locked`。
Rust 对 `unwrap()` 和 `expect()` 启用 Clippy deny；应用启动失败会输出错误并返回非零退出码。
前端处理 IPC 拒绝、非桌面环境和 8 秒超时，支持重新检查；超时只结束等待，不取消已提交的 Rust 工作。

打包文件位于 `src-tauri/target/release/bundle/`。各平台分别在目标系统上构建和验证。
Phase 1 不配置发布签名、macOS 公证或自动更新；当前应用标识 `dev.bilidm.player` 是开发标识。
如需更换图标，修改 `assets/app-icon.svg` 后运行：

```sh
npm run tauri -- icon assets/app-icon.svg
```

图标命令同时生成的 Android/iOS 目录已忽略，当前仅维护桌面图标。

## 目录与职责

```text
src/
  main.tsx                      React 入口
  frontend/
    App.tsx                     应用组合
    pages/                      页面组合
    components/                 展示组件
    hooks/                      页面状态与生命周期
    stores/                     预留跨页面状态（当前无需全局状态库）
    services/                   前端用例服务
    lib/desktop.ts              Tauri IPC 唯一入口、超时与错误整理
    types/                      前后端传输类型
    styles.css                  基础界面样式
src-tauri/
  src/
    main.rs                     桌面进程入口与启动错误处理
    lib.rs                      依赖装配、状态注册、命令注册
    commands/                   Tauri 命令适配
    core/                       独立于 Tauri 的 Rust 应用服务
    torrent/                    预留 torrent engine 边界
    media/                      预留媒体模型与文件识别
    player/                     预留播放器接口与 mpv 适配
    database/                   预留 SQLite 仓储与迁移
  capabilities/                 主窗口能力配置
  icons/                        桌面平台图标
  Cargo.toml / Cargo.lock        Rust 依赖与锁文件
  tauri.conf.json                窗口、构建、CSP、打包配置
plugins/README.md                未来扩展约定；无插件运行时
docs/architecture.md            模块依赖与后续数据流约定
docs/phase-1.md                 本阶段文件清单、验证记录、已知问题
```

调用方向：**Frontend → frontend service → Tauri Command → Rust Core service → 具体适配器**。
当前真实用例为 `HomePage → useRuntimeInfo → runtimeService → desktop → get_runtime_info → AppService`。
组件不直接调用 torrent、mpv、数据库或 Tauri。
Tauri 启动层负责管理服务生命周期；Core 不引用 Tauri 类型。
生产 CSP 限制资源到应用本身和本地 IPC；开发 CSP 额外允许 Vite 本地热更新。

## 分阶段交付

| 阶段 | 范围 | 状态 |
| --- | --- | --- |
| Phase 1 | Tauri + React + Rust、IPC 验证、模块骨架、工程说明 | 当前交付 |
| Phase 2 | torrent engine、磁力链接解析、metadata 与文件列表 | 已完成 |
| Phase 3 | mp4/mkv/avi/webm/mov 识别、默认选择最大视频 | 未实现 |
| Phase 4 | mpv/libmpv、边下载边播放、暂停和 seek | 未实现 |
| Phase 5 | 播放器基础 UI、音量、进度、下载速度与进度 | 未实现 |
| Phase 6 | SQLite torrent 任务、媒体持久化与迁移 | 未实现 |

每一阶段单独提交并更新文件清单、架构说明、测试方法与已知问题。
后续扩展允许弹幕、metadata、外部 Agent 的 `match.json`，Phase 1 仅保留边界，不实现插件系统。

## Phase 1 手工验收

1. 运行 `npm run desktop:dev`，确认原生窗口正常渲染，没有白屏。
2. 确认“桌面服务已连接”及版本、系统、CPU 架构由 Rust 返回。
3. 点击“重新检查”，确认恢复连接状态；调整窗口到最小尺寸，检查内容可用。
4. 关闭桌面开发任务；运行 `npm run dev` 并用普通浏览器访问，确认显示桌面环境提示，不伪造成功状态。
5. 运行 `npm run check` 和 `npm run tauri -- build --no-bundle`；构建后直接启动二进制，确认不依赖 Vite 仍能渲染和调用 Rust。

本机实际完成哪些检查及平台限制，以 [Phase 1 报告](docs/phase-1.md) 为准。

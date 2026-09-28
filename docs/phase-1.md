# Phase 1 交付记录

日期：2026-09-28。范围：初始化 Tauri 2 + React + TypeScript + Vite + Rust，验证桌面启动及真实 IPC。

## 当前架构

`HomePage → useRuntimeInfo → runtimeService → desktop IPC → commands/runtime → core/AppService`。

- `lib.rs` 装配 Tauri 状态及命令，`main.rs` 处理启动失败与退出码。
- Core 无 Tauri 依赖；前端只有 `lib/desktop.ts` 引用 Tauri API。
- torrent、media、player、database 仅预留独立模块，未引入后续阶段依赖。
- `plugins/README.md` 记录未来扩展契约，不存在插件系统实现。
- TypeScript strict，Rust Clippy 禁止 unwrap/expect，锁定 npm/Cargo 依赖解析结果。

详细职责和未来流式播放的边界见 [架构说明](architecture.md)。

## 修改文件列表

初始目录为空，以下文件均为本阶段新增。

```text
.editorconfig
.gitignore
README.md
assets/app-icon.svg
docs/architecture.md
docs/phase-1.md
index.html
package-lock.json
package.json
plugins/README.md
rust-toolchain.toml
tsconfig.json
tsconfig.node.json
vite.config.ts
src/main.tsx
src/frontend/App.tsx
src/frontend/components/AppHeader.tsx
src/frontend/components/RuntimeStatus.tsx
src/frontend/hooks/useRuntimeInfo.ts
src/frontend/lib/desktop.ts
src/frontend/pages/HomePage.tsx
src/frontend/services/runtimeService.ts
src/frontend/stores/README.md
src/frontend/styles.css
src/frontend/types/runtime.ts
src-tauri/Cargo.toml
src-tauri/Cargo.lock
src-tauri/build.rs
src-tauri/tauri.conf.json
src-tauri/capabilities/default.json
src-tauri/src/main.rs
src-tauri/src/lib.rs
src-tauri/src/commands/mod.rs
src-tauri/src/commands/runtime.rs
src-tauri/src/core/mod.rs
src-tauri/src/core/app_service.rs
src-tauri/src/torrent/mod.rs
src-tauri/src/media/mod.rs
src-tauri/src/player/mod.rs
src-tauri/src/database/mod.rs
src-tauri/icons/32x32.png
src-tauri/icons/64x64.png
src-tauri/icons/128x128.png
src-tauri/icons/128x128@2x.png
src-tauri/icons/icon.png
src-tauri/icons/icon.icns
src-tauri/icons/icon.ico
src-tauri/icons/StoreLogo.png
src-tauri/icons/Square30x30Logo.png
src-tauri/icons/Square44x44Logo.png
src-tauri/icons/Square71x71Logo.png
src-tauri/icons/Square89x89Logo.png
src-tauri/icons/Square107x107Logo.png
src-tauri/icons/Square142x142Logo.png
src-tauri/icons/Square150x150Logo.png
src-tauri/icons/Square284x284Logo.png
src-tauri/icons/Square310x310Logo.png
```

`node_modules/`、`dist/`、Rust `target/`、Tauri 生成的 schema 均为被忽略的本地构建产物。

## 测试方法与记录

环境：macOS 26.5.2 / Apple Silicon aarch64；Node.js 22.23.2；Rust 1.98.1；Tauri 2.12.0；React 19.3.0；TypeScript 7.0.2；Vite 8.3.1。

| 检查 | 方法 | 结果 |
| --- | --- | --- |
| 前端类型与生产构建 | `npm run build` | 通过；两个 tsconfig 均开启 strict |
| Rust 格式 | `npm run rust:fmt` | 通过 |
| Rust 静态检查 | `npm run rust:clippy` | 通过；all-targets、locked、warnings deny |
| 桌面开发进程 | `npm run desktop:dev` | 已完成编译并启动原生进程 |
| 浏览器环境对照 | 浏览器访问 127.0.0.1:1420，点击重新检查 | 明确提示启动桌面应用，没有伪造 Rust 信息 |
| macOS 生产应用 | `npm run tauri -- build --bundles app` | 通过；生成 9.48 MiB 的 Bili DM.app |
| 生产应用真实 IPC | 停止 Vite 后启动 .app，检查运行信息及重新检查按钮 | 通过；tauri://localhost 窗口显示 Bili DM / 0.1.0 / macos / aarch64，点击后仍正确返回 |
| 界面目视检查 | 原生窗口截图与可访问性树 | 通过；默认窗口完整展示启动页、运行状态和按钮 |

本阶段没有 torrent、播放器或持久化逻辑；未添加仅复述常量的单元测试。验收依据为编译、Clippy、实际窗口和 IPC。

生产产物：`src-tauri/target/release/bundle/macos/Bili DM.app`。
桌面开发任务和临时浏览器页已关闭，1420 端口不再监听；生产应用已独立启动供查看。

复现步骤详见 [README](../README.md) 的“Phase 1 手工验收”。

## 已知问题与验证边界

- Windows、Linux 采用相同代码和各平台 Tauri 配置，但本次没有对应系统的构建/运行证据；不能视为已验收。
- 开发命令的编译和进程启动已验证；开发裸二进制未被 UI 工具识别，原生界面与 IPC 的直接观察证据来自生产 .app。
- 磁力解析、文件识别、播放控制、下载统计、SQLite 分别等待 Phase 2–6，本阶段没有这些能力。
- mpv/libmpv 的嵌入方式、打包依赖和 torrent 的随机 seek 支持尚未选型验证。
- 当前仅开发标识和本地应用构建，没有签名发布、公证、Windows 安装包或 Linux 安装包验收。
- Rust/TypeScript DTO 暂手工保持一致；未来命令增多后再评估类型生成。
- IPC 超时仅停止前端等待，不取消 Rust 任务。后续下载/播放操作需要按用例设计取消与进度事件。

## 本地环境改动

安装 Rust stable + rustfmt + clippy 到标准用户目录；未修改 Shell 启动文件。
初始化本地 Git 仓库，未配置远端、未推送。依赖及构建缓存留在标准目录供后续阶段复用。

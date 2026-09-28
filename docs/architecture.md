# 模块边界

## Phase 1 的实际实现

```text
React page / presentation components
       ↓ hook（生命周期 / 连接状态）
frontend/services/runtimeService.ts
       ↓ frontend/lib/desktop.ts（IPC）
Rust commands/runtime.rs
       ↓ State<AppService>
Rust core/app_service.rs
```

`main.rs` 处理进程结果，`lib.rs` 是装配入口，负责注册服务和命令。
`AppService` 提供不依赖 Tauri 的运行信息；返回值按 camelCase 序列化。
前端传输类型集中在 `types/runtime.ts`，修改 Rust DTO 时必须同步更新。
当前命令只读编译期及平台常量，本身无可恢复业务错误；Tauri 启动和前端 IPC 失败均有错误处理。
业务阶段引入的可失败命令使用 `Result<T, AppError>`，错误通过可序列化的 code/message 传输，组件只展示可理解的信息。

`torrent`、`media`、`player`、`database` 是编译中的空模块，仅有职责文档。
目前没有 engine、数据库连接、假实现、下载任务或插件加载器。
不提前添加未经验证的异步 trait、引擎选型或全局状态库。

## 后续阶段的接口方向（设计约定，尚未实现）

- `torrent`：以引擎无关的任务 ID、文件描述、下载状态和可定位字节数据对外提供能力。引擎实现封装在该模块；不暴露具体引擎对象到命令或前端。
- `media`：定义 `MediaIdentity`、媒体描述和文件选择策略。Phase 3 的扩展名和最大文件规则在这里实现，避免出现在 UI 中。
- `player`：定义 `PlayerBackend` 接口，再提供 mpv/libmpv 适配。只接收 Core 提供的媒体源与播放指令，不解析 magnet、不操作 torrent 实例。
- `core`：组合任务、媒体和播放器用例。负责资源关闭、取消与服务生命周期协调；通过接口依赖适配器，不让 Command 承担业务编排。
- `database`：SQLite 连接、迁移及任务/媒体仓储。前端不得执行 SQL，迁移必须版本化。

预期播放数据流：

```text
Torrent adapter → seekable media source → mpv adapter
                        ↑
                 Core playback service
                        ↑
                 Tauri Command / events
                        ↑
                    Frontend
```

随机拖动需要按所需范围请求数据并提升 piece 优先级，不能仅把未完整下载的文件路径交给 mpv 后宣称支持流式 seek。
引擎和媒体源协议的选型在 Phase 2/4 实测后确定；视频内容不通过 JSON IPC 搬运。
如果需要本机环回流媒体传输，它只能是进程内部媒体传输适配，不能引入云端或业务后端 API。

## 持久化预留

Phase 6 的最低数据模型为 torrent 任务（torrent_id、magnet_uri、created_at、status）和媒体（id、path、filename、duration）。
缓存与数据库放入操作系统应用数据/缓存目录，通过启动层将路径传入服务，不能写死用户目录。
未来 metadata、弹幕来源和匹配关系使用关联表及媒体 ID，避免把提供方字段直接混入播放核心。
本阶段不创建表或数据库文件。

## 扩展边界

未来扩展以媒体身份作为输入，不依赖 `PlayerComponent` 或 torrent 引擎内部对象：

- 弹幕扩展：`MediaIdentity` → 带时间戳的弹幕流。
- metadata 扩展：`MediaIdentity` → 标题、封面、简介。
- 匹配配置适配：外部 Agent 生成 `match.json`，核心只读取并校验配置，不执行 AI 或自动匹配。

未来实现时通过显式接口和装配入口连接；本阶段没有插件扫描、安装、热加载或执行能力。

# 当前架构（0.2.0）

```text
React 页面 / 展示组件
    ↓ hooks（异步状态、轮询、交互互斥）
frontend/services → lib/desktop.ts（IPC、错误、超时）
    ↓
Tauri commands（参数 / 系统文件选择适配）
    ↓
AppService / PlaybackService（用例、生命周期、持久化编排）
    ├── TorrentEngine → RqbitEngine
    ├── PlayerBackend → MpvBackend
    ├── LibraryRepository → SqliteLibrary
    └── MediaServer → 单文件 HTTP Range

视频字节：rqbit FileStream（AsyncRead + AsyncSeek）→ loopback → mpv
本地播放：规范化路径 → mpv
```

lib.rs 是依赖装配入口，通过 Tauri 取得系统目录；Core 不引用 Tauri 类型。Command 不处理 SQL、torrent metadata 或 mpv JSON 命令。PlayerControls 只接收状态和回调，不依赖 torrent。

## 模块契约

- TorrentEngine：解析/取消 metadata、准备选中文件、可 seek reader、下载统计、暂停/退出。librqbit 类型只在适配器内。list-only 解析不下载正文；用户选片后启动下载，seek 通过 reader 调整 piece 需求。
- media/files：扩展名分类、最大非空视频及稳定文件 index。UI 自然排序不改变文件身份。
- PlayerBackend：媒体源、控制和快照。mpv 用 Unix socket / Windows named pipe JSON IPC；不理解 magnet 或数据库。
- PlaybackService：一次一个播放会话，串行处理切换、控制和快照；停止撤销媒体源、暂停下载、更新状态；退出关闭子进程与会话。时长绑定同一媒体快照，成功读取后保存一次。
- LibraryRepository：保存目录、媒体和任务状态，读取历史。SQLite 用 spawn_blocking 执行，连接互斥、外键、WAL、参数绑定及事务。迁移由 user_version 管理，拒绝更新版本 schema。

## 数据传输

mpv 访问 http://127.0.0.1:<随机端口>/stream/<随机 token>。支持 GET/HEAD、单范围 Range、suffix ranges 与 416；seek 定位字节 reader。端点不接受任意磁盘路径；切换替换 token，停止后原 URL 返回 404。

该端点只是进程内部的媒体传输，不提供业务后端 API；前端不能用它管理任务。视频字节不经过 Tauri IPC。不存在账号、云服务或业务网络请求。

## 持久化与身份

torrent_tasks 保存磁力、目录快照、创建时间与 ready/downloading/paused 状态；media 保存路径、文件名、可空时长及 torrent/file 引用。本地媒体以规范化绝对路径构造 ID，磁力媒体以 info hash + file index 构造 ID，路径表示为 torrent://<hash>/<index>。

重启将 downloading 恢复为 paused；读取历史不创建网络会话。内存 metadata 对象不持久化，重新播放需重新解析，正文缓存则校验复用。移动本地文件会形成新路径身份，未来内容去重应通过专门适配器实现。

## 字幕和未来扩展

FileKind::Subtitle 已识别外部字幕，当前 prepare_file 仍只接受 Video。后续分别提供视频源和字幕源，在 PlayerBackend 添加字幕挂载/移除操作；本地字幕用系统选择器，磁力字幕用独立下载/流源。**不能用当前唯一 active 视频槽加载字幕**，否则会中断视频。

未来以稳定媒体 ID 为输入，独立接口接入弹幕流、标题/封面/简介及外部 Agent 的 match.json。新增关联表保存 provider 与媒体关系，核心表不加入 Bilibili 或 AI 专有字段。当前没有插件扫描、安装、执行或自动匹配；约定见 plugins/README.md。

## 错误和验证

统一 AppError { code, message }，command 返回 Result，前端显示错误。metadata 可取消且最长 120 秒，缓存初始化、播放打开、IPC 有独立时限。前端时限不等于后台取消。

测试分层：纯文件/Range/magnet 校验、真实 loopback HTTP、SQLite 持久化/迁移/事务、真实 mpv/librqbit smoke、原生 GUI。平台验证以阶段报告为准。

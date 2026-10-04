# 当前架构（0.3.0）

```text
React 页面 / 展示组件
    ↓ hooks（串行轮询、操作互斥、过期响应丢弃）
frontend/services → Tauri commands（参数、系统对话框、原生窗口）
    ↓
AppService / PlaybackService（队列、续播、生命周期）
    ├── TorrentEngine → RqbitEngine
    ├── PlayerBackend → MpvBackend → libmpv C API
    ├── LibraryRepository → SqliteLibrary
    └── MediaServer → 单文件 HTTP Range

字节通道：rqbit FileStream → loopback HTTP → libmpv
显示通道：libmpv 原生视图 → 透明 WebView 下方
控制通道：React DTO → Command → Core → PlayerBackend
```

lib.rs 装配依赖、获取系统目录和窗口句柄、启动后台监视器。Core 不引用 Tauri 类型，Command 不解析 torrent、不处理 SQL 和播放器协议。

## TorrentEngine

Session 按需创建，无启动恢复和自动联网。list_only 解析 metadata，支持取消和 120 秒超时；有效种子元数据原子写入本地缓存。新增种子以 paused + 空 only_files 注册，完成缓存校验后再选择当前视频及匹配字幕。准备失败或取消由 Core 清理。

media/playlist 提供自然排序和字幕匹配，不改变 engine index。字幕使用独立 reader 请求分片，仅在校验完成后把本地路径交给 libmpv，不占用视频的 active slot。VobSub 等待 idx/sub 成对就绪。

视频 reader 的 seek 请求交给 rqbit 的流优先级机制。统计读取真实 have bitfield，映射为文件内归一化连续区间。下一集仅在当前视频全部下载完成后加入 only_files；关闭预取移除下一集，暂停/停止取消字幕读取和下载。一次只有一个活动视频。

## PlayerBackend

动态加载 libmpv 稳定 C API；Library 生命周期覆盖 mpv handle。所有 handle 操作由 Mutex 串行化并在 spawn_blocking 执行，事件指针在下一次 poll 前读取、节点树读取后释放。没有外部播放器进程、socket 或 named pipe。

真实 app 传入 NSView/Win32/X11 窗口句柄，使用 gpu-next；CLI smoke 使用 vo=null、ao=null 进行解码验证。native view 在 WebView 下方，通过 video-margin-ratio-* 跟随页面的 ResizeObserver。macOS 标题栏使用 Overlay，确保两个视图坐标一致；sub-use-margins=no 将字幕限制在视频范围内。

macOS 应用保持 hardened runtime，并仅为本应用声明 disable-library-validation entitlement，以加载不同签名身份的 libmpv/LGPL 动态库。库路径固定在应用资源目录，开发模式回退到工作区 lib；不暴露前端任意加载库的命令。

播放器禁用用户脚本和 mpv 自带的 Lua 控制台、统计面板、自动配置等脚本：这些功能已由本应用承担，也避免 LuaJIT 执行内存触发 hardened runtime 的签名终止。无需开放 unsigned-executable-memory。

暴露 load、控制、外挂字幕、视口、快照和 shutdown。快照包含位置/时长、暂停/EOF、音量/倍速、音轨/字幕、章节、解码和缓冲状态。音轨偏好按语言/标题/编码匹配，避免跨文件复用不同含义的 track ID。

## PlaybackService

Session mutex 串行化切换和状态变更，独立 published snapshot 保证打开/缓存校验期间 UI 状态读取不被阻塞。500 ms 后台 tick 负责自动字幕、偏好恢复、进度保存、预取和 EOF；不依赖前端轮询驱动播放。

打开操作有 cancellation token：停止先取消准备，再撤销 HTTP 源、停止 mpv、暂停 torrent 和保存状态。native spawn_blocking load 不通过丢弃 future 取消，必须等待返回后清理，避免停止之后又开始播放。退出保持 AppKit 事件循环运行，后台销毁播放器并关闭 BT 会话后再退出进程。

EOF 支持单集/列表循环与自动下一集；无后续集时暂停下载但保留可回看的媒体。当前集完成前不预取，最多一个后续文件。失败会停止相关下载并发布可见错误。

## HTTP 传输

端点只监听 127.0.0.1 随机端口，路径含随机 token 并绑定当前选中的文件；没有任意磁盘路径或管理 API。支持 GET、HEAD、单范围 Range、suffix、416。打开 reader 最长等待 15 秒。

切换或停止会使旧 URL 失效，并主动取消已经建立但等待缺失分片的响应体，释放 reader 和读取优先级。媒体字节不经过 JSON。缓冲条按字节比例显示，有别于实际解码时间映射。

## 持久化与身份

schema 2 保留 torrent_tasks 和 media，增加以规范化 path 为外键的 playback_progress，以及单行 JSON playback_preferences。进度包含 position、duration、completed；本地媒体按规范化绝对路径识别，torrent 媒体按 info hash + file index 识别。

SQLite 连接使用 Mutex、spawn_blocking、WAL、外键、参数绑定及迁移事务，拒绝未知新 schema。重启将 downloading 标记为 paused；读取本地历史、进度和偏好不会创建引擎 Session。

## 验证与扩展

21 个自动化测试覆盖纯函数、真实 HTTP、SQLite 迁移和播放状态机。streaming_smoke 验证受控真实 BT + libmpv；原生 app 单独验证画面嵌入、字幕和窗口生命周期。浏览器页面、headless 解码、原生显示、公网节点和其他操作系统分别作为不同验证边界。

后续弹幕、匹配和媒体信息模块应通过稳定媒体身份接入，不向 torrent/player/database 适配器加入 Bilibili 或 AI 业务字段。插件约定仍见 plugins/README.md。

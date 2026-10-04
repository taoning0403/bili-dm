# 内置弹幕混合插件

当前通过编译期模块 `src-tauri/src/danmaku` 接入 Bilibili，不加载任意第三方脚本或动态插件。`DanmakuProvider` 契约支持解析 BV/分 P、读取当前分段弹幕池、取得短期视频流地址。

流程：React → frontend service → Tauri command → DanmakuService → provider / 通用 FrameProbe / 纯函数匹配与混合。Core 用稳定媒体身份、播放会话 ID 和撤销令牌提供只读分析源。独立 libmpv 实例取帧，不跳转主播放器。Bilibili 业务字段不进入 TorrentEngine / PlayerBackend / LibraryRepository。

混合结果保存为版本化 JSON 项目和标准 Bilibili XML。Canvas 按播放器时间显示弹幕，不占用字幕轨。区间契约示例：

```json
{
  "sourceId": "BV1xx411c7mD:62131",
  "sourceStart": 10,
  "sourceEnd": 70,
  "targetStart": 0,
  "targetEnd": 60,
  "enabled": true
}
```

详见 [弹幕混合说明](../docs/danmaku-mixer.md)。后续来源可实现同一 provider 契约；第三方插件发现、任意脚本执行、账户与云同步尚未实现。

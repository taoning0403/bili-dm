# 未来扩展位置

此目录目前只有设计说明，不包含插件系统或插件实现。

未来可按 `danmaku-plugin/`、`metadata-plugin/`、`ai-match-plugin/` 拆分。
扩展通过媒体身份和稳定的数据契约与 Core 通信，不能直接依赖具体 torrent 引擎、mpv 实例或前端组件。
AI 匹配能力由外部 Agent 提供，播放器仅读取和校验结果配置，例如：

```json
{
  "media": "xxx",
  "bilibili_id": "BVxxxx",
  "offset": 12.4,
  "confidence": 0.98
}
```

此示例不是已接入的功能。schema 版本、字段验证和插件权限在后续扩展阶段定义。

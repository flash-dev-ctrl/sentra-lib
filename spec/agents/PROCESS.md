# 进程资产

进程资产用于采集本机正在运行的 agent 进程，并通过 `sentra list process` 暴露。

```json
{
  "pid": 1234,
  "name": "codex.exe",
  "cmdline": ["codex.exe", "--sandbox"],
  "path": "C:\\Users\\me\\AppData\\Local\\Programs\\OpenAI\\Codex\\codex.exe",
  "env": {
    "OPENAI_API_KEY": "sk-****7890"
  }
}
```

边界：

- 采集范围保持在已知 agent 进程匹配规则内，避免按模糊关键词扩大误报。
- 当前覆盖 Codex、Claude、OpenCode、OpenClaw、Pi、Hermes、Sentra 等 agent 的运行中进程。
- 只支持列表展示，不支持 `scan process`、停止进程或后台监控。
- `env` 默认脱敏，命中 key/token/secret/password/credential/auth/bearer/session/cookie/private 的变量不输出原值。
- C binding 的 `collect_all_assets` 暂不默认包含进程资产。

进程采集作为独立资产入口提供，不嵌入 agent 对象。

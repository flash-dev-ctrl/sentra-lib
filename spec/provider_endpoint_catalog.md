# Provider Endpoint Catalog 能力调查

调查日期：2026-07-13

## 目标

将 Provider Catalog 建设成已知 LLM 厂商官方 Endpoint 的版本化知识库，为 Agent 资产归一化、官方直连识别和疑似中转站检测提供统一依据。

## 数据源与现状

- 本地 Pi `0.80.3` 的 `@earendil-works/pi-ai` 注册了 35 个内置 Provider 条目。条目中既有厂商，也有区域、套餐、OAuth 和协议入口，例如 `minimax` / `minimax-cn`、`zai` / `zai-coding-cn`、`xiaomi-token-plan-*`。
- OpenCode 通过 AI SDK 与 Models.dev 支持大量 Provider，并允许任意自定义 Provider。离线快照包含 159 个 Provider/区域/套餐条目；2026-07-13 实时审计为 162 个。
- 原 Catalog 包含 17 个 Canonical Provider。它能够保留未知或自定义 Provider，但部分已知 Provider 缺少官方 Endpoint，且 `resolutionStatus=known` 不能说明显式 URL 是否官方。
- 当前分支已将实时 162 个 Models.dev Provider 条目全部映射到 137 个 Canonical Provider；134 个静态 API Endpoint 全部可识别。Models.dev 地址默认标记为 `models_dev`，不会自动提升成厂商官方地址。

参考：

- Pi 内置 Provider 文档：`docs/providers.md`（随 Pi 安装包发布）
- OpenCode Provider 文档：https://dev.opencode.ai/docs/providers
- Models.dev API：https://models.dev/api.json
- Models.dev 数据仓库：https://github.com/anomalyco/models.dev

## 核心结论

### Canonical Provider 表示实际厂商

区域、品牌、套餐和协议不应重复计算厂商：

| Canonical Provider | Endpoint / Alias 示例 |
| --- | --- |
| `zai` | Zhipu AI、Z.AI、GLM、国内/国际、Coding Plan |
| `minimax` | MiniMax 国内/国际、OpenAI/Anthropic 兼容入口 |
| `moonshotai` | Moonshot 国内/国际、Kimi For Coding |
| `xiaomi` | MiMo API、CN/AMS/SGP Token Plan |
| `alibaba` | 百炼/DashScope、Coding Plan、Token Plan、国内/国际 |
| `tencent` | 混元、Coding Plan、Token Plan、TokenHub |

OpenRouter、OpenCode Zen、Vercel AI Gateway 等拥有独立认证与官方 Endpoint 的网关作为独立 Canonical Provider 保留。

### Provider 身份与 Endpoint 路由必须分开

Provider ID 只能证明配置声称使用哪个厂商，不能证明请求直连官方地址。解析输出需要同时表达：

- `resolutionStatus`：Canonical Provider 身份是否已知。
- `routeStatus`：已观测 Endpoint 是否经过厂商验证、仅来自二级数据源，或是否为疑似中转。
- `endpointTrust`：`vendor_verified`、`models_dev` 或 `unverified`；未观测到 URL 时为空。

示例：

| Provider ID | Base URL | Provider 身份 | Route |
| --- | --- | --- | --- |
| `openai` | `https://api.openai.com/v1` | known | official |
| `stackit` | Models.dev 收录地址 | known | unverified |
| `openai` | `https://relay.example/v1` | known | relay_candidate |
| `openai` | `https://api.deepseek.com` | known | provider_mismatch |
| `corp-gateway` | `https://relay.example/v1` | custom | custom |

### Catalog 地址必须保留信任来源

第三方中转地址不加入上游厂商的 Endpoint 列表。已知中转平台若有独立认证和域名，应作为独立 Gateway Provider；未知中转地址保留原始 URL 并标记为 `relay_candidate` 或 `custom`。Models.dev 用于发现候选地址和覆盖率回归，但只有经过厂商资料复核的条目才能标记为 `vendor_verified`。

## 匹配策略

当前阶段使用 Canonical Base URL 和已知等价 Base URL：

1. Agent 显式 URL 优先，Catalog 不覆盖。
2. 显式 URL 命中 `vendor_verified` Endpoint 时标记 `official`；仅命中 Models.dev/未验证条目时标记 `unverified`。
3. Provider ID 已知但显式 URL 未命中时标记 `relay_candidate`；允许自定义 Endpoint 的 Provider 标记 `unverified`。
4. Provider ID 与 Endpoint 分别命中不同厂商时标记 `provider_mismatch`。
5. 未知 ID 与未知 URL 保留为 `custom`。

动态云 Endpoint 使用 URL 解析后的受约束 Host/Path 规则，避免使用不受控正则。当前已覆盖 Cloudflare Workers AI / AI Gateway、Databricks AI Gateway 和 Snowflake Cortex，并拒绝凭据、Query、Fragment、非默认端口、伪造域名后缀和 Path 边界混淆。后续可按厂商资料继续增加：

- Azure OpenAI 的资源子域名。
- Amazon Bedrock 的地域 Endpoint。
- Google Vertex AI 的地域 Endpoint。
- 阿里云百炼业务空间专属域名。

匹配优先级应为：精确 Base URL > 已知 URL Alias > 精确 Host + Path Prefix > 受约束的官方域名模板。跨厂商同分时返回 `ambiguous`。

## 实施范围

第一阶段：

- 增加独立 `routeStatus`，修复显式中转 URL 仍沿用默认官方 Endpoint Variant 的问题。
- Endpoint 协议改为可选，使 Catalog 能收录尚未支持主动探测的官方 API。
- 支持 Endpoint Base URL Alias。
- 补齐本地 Pi 内置的固定 Endpoint，以及 DeepSeek、Kimi、智谱、MiniMax、火山方舟、阿里百炼、腾讯混元、百度千帆、OpenRouter、OpenCode 和小米 MiMo。
- Pi 从配置、认证文件或环境变量读取的 API Key 在进入 `ProviderData` 前统一脱敏。
- 将 API Key/Token 环境变量与 Host、Account、Profile、凭据文件路径等普通配置变量分开。
- 增加 Endpoint 信任等级和受约束的动态 Host/Path 匹配。

后续阶段：

- 按厂商资料继续补齐其他云平台的动态 Host/Path 规则。
- 将 route 状态接入 Provider 风险规则，而不是仅把 URL 文本交给通用扫描器。
- 增加 Catalog 更新脚本；Models.dev 可作为候选数据源，但需要进行厂商归并和官方文档复核，不能直接把其区域/套餐条目当作 Canonical Provider。
- 增加 Catalog 覆盖率报告，持续对照 Pi、OpenCode/Models.dev 和其他 Agent 的内置 Provider。

仓库提供两种覆盖率保障：

- `tests/fixtures/models-dev-provider-snapshot.json` 保存调查时的 159 个 Provider ID 与 API 地址，离线回归测试确保身份和静态 Endpoint 均可解析。
- `scripts/audit-provider-catalog.sh` 对比实时 Models.dev API，用于发现上游新增 Provider 或 Endpoint 漂移；该脚本需要网络和 `jq`，不加入默认离线测试。

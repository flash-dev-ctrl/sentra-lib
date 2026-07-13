RiskChecker
    new(
        options: ScanOptions,
        rules: RuleStore,
        cache: CheckResultCache
    )

修改cli子命令：scan
当前只需要实现skill即可，但是要考虑扩展性,配置文件在 ~/.sentra/config.json
逻辑时先采集资产然后调用RiskScann.scan

sentra scan skill(cron, memory, xxx) --agent codex --agent claude

默认扫描所有agent

支持
xxx == hash yara ti llm online-ti
--with-xxx
--without-xxx

默认 --with-hash --with-yara --with-ti
并通过with-xxx，扩展
如果有without-xxx，则从 with 中删除 xxx

## Provider Registry

LLM Provider 扫描分为 Agent 配置解析与公共供应商知识解析两层。Agent Adapter 只负责读取本地事实；Provider Registry 负责标准化供应商身份、补全缺失 Endpoint、选择协议变体并标记字段来源。

Catalog 以 Endpoint 为核心：Canonical Provider 表示实际厂商，区域、套餐、Coding Plan 和协议作为 Endpoint Variant，不重复创建厂商。Endpoint 同时记录 `vendor_verified`、`models_dev` 或 `unverified` 信任来源；Provider 身份与路由官方性分别判定，避免已知 Provider ID 或二级数据源掩盖第三方中转地址。

```mermaid
flowchart LR
    subgraph Agents["Agent Adapter"]
        Pi["Pi"]
        OpenClaw["OpenClaw"]
        Codex["Codex"]
        Claude["Claude CLI / App"]
        Hermes["Hermes"]
        Sentra["Sentra"]
        Future["OpenCode / Cherry Studio / Future Agents"]
    end

    Pi --> Candidate
    OpenClaw --> Candidate
    Codex --> Candidate
    Claude --> Candidate
    Hermes --> Candidate
    Sentra --> Candidate
    Future --> Candidate

    Candidate["ProviderCandidate<br/>agentProviderId?<br/>configuredBaseUrl?<br/>protocolHint?<br/>credential / models<br/>activation: active | inactive | unknown"]

    subgraph Registry["Public Provider Registry"]
        Alias["Alias Resolver<br/>Agent Alias -> Canonical Provider"]
        Endpoint["Endpoint Resolver<br/>Region + API Variant"]
        Reverse["Endpoint Reverse Lookup"]
        Enrich["Metadata Enrichment<br/>Display Name / URL / Protocol"]
        Provenance["Provenance<br/>configured | catalog | inferred"]
    end

    Candidate --> Alias --> Endpoint --> Enrich --> Provenance
    Candidate --> Reverse --> Endpoint

    subgraph Catalog["Versioned Provider Catalog"]
        Providers["Canonical Providers"]
        Aliases["Global + Agent-scoped Aliases"]
        Endpoints["CN / Global<br/>Responses / Chat / Anthropic"]
        Auth["Credential + Configuration Key Names"]
        Trust["Endpoint Trust<br/>vendor_verified | models_dev | unverified"]
    end

    Providers --> Alias
    Aliases --> Alias
    Endpoints --> Endpoint
    Auth --> Enrich
    Trust --> Endpoint

    Provenance --> Resolved["ProviderData<br/>providerId?<br/>rawProviderId?<br/>endpointVariant?<br/>resolutionStatus<br/>routeStatus<br/>activationStatus"]
    Resolved --> Inventory["Inventory UI"]
    Resolved --> Risk["Provider Risk Scanner"]
    Resolved --> Probe["Model Probe"]
    Resolved --> Bindings["Rust / C / TS Bindings"]
```

解析优先级：

1. Agent 配置中的显式 URL 和协议，不允许 Catalog 覆盖。
2. Agent-scoped Alias，可同时指定 Canonical Provider 和 Endpoint Variant。
3. Canonical Provider 或全局 Alias 的默认 Endpoint。
4. 根据显式 Endpoint 反向识别供应商；只在唯一匹配时采用。
5. 无法识别时保留原始条目，不猜测或丢弃。

路由状态独立于供应商身份：

- `official`：观测到的显式 URL 命中经过厂商资料验证的 Endpoint。
- `unverified`：显式 URL 命中 Models.dev/未验证条目，或该 Provider 明确允许自定义 Endpoint。
- `relay_candidate`：Provider 身份已知，但显式 URL 不属于该厂商的官方 Endpoint。
- `provider_mismatch`：Provider ID 与显式 URL 分别命中不同的 Canonical Provider。
- `custom`：显式 URL 和 Provider ID 均无法归入已知厂商。
- `ambiguous`：显式 URL 同时命中多个 Canonical Provider。
- `unknown`：未观察到显式 Endpoint，或缺少足够信息；Catalog 默认 URL 的补全不作为“当前正在官方直连”的证据。

识别状态：

- `known`：Catalog 中唯一匹配的供应商。
- `custom`：存在显式 Endpoint，但 Catalog 未收录。
- `unknown`：存在 Provider ID，但缺少足够信息进行识别。
- `ambiguous`：多个 Catalog 条目均可能匹配。

边界约束：

- Catalog 不保存真实 API Key，只维护认证方式所需的环境变量名等元数据；凭据变量与 Host、账号、配置文件路径等普通配置变量分开建模。
- Models.dev 是覆盖率和候选地址来源，不是官方信任根。只有经过厂商资料复核的 Endpoint 才能标记为 `vendor_verified` 并产生 `official` 路由状态。
- 动态云 Endpoint 使用 URL 解析后的 HTTPS Scheme、受约束 Host/域名后缀和 Path 规则匹配，不使用不受控正则；凭据、Query、Fragment、伪造域名后缀和非默认端口均不能命中。
- Canonical Provider 不按品牌、地区、套餐或协议重复拆分。例如智谱/Z.AI、MiniMax 国内/国际入口均使用同一个 Provider ID。
- `activationStatus` 由 Agent Adapter 决定；Registry 不推测当前激活项。
- Agent 的 Provider 写入和删除逻辑继续归各 Adapter 所有，Registry 只负责标准化与补全。
- 同一 Base URL 可以对应多个协议变体，供应商身份识别和 Endpoint Variant 识别必须分别处理。
- Catalog 作为仓库内版本化快照发布，运行时不依赖网络更新。

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

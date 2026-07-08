use crate::interfaces::{RiskCategory, RiskSeverity};

pub(crate) mod messages {
    pub const HASH_CHECKER_NAME: &str = "Hash Checker";
    pub const HASH_CHECKER_DESCRIPTION: &str =
        "Checks content hashes against local allow and deny lists.";

    pub const YARA_CHECKER_NAME: &str = "YARA Checker";
    pub const YARA_CHECKER_DESCRIPTION: &str = "Scans content with configured YARA rules.";

    pub const THREAT_INTEL_CHECKER_NAME: &str = "Threat Intelligence Checker";
    pub const THREAT_INTEL_CHECKER_DESCRIPTION: &str =
        "Checks network indicators against local threat intelligence feeds.";

    pub const LLM_CHECKER_NAME: &str = "LLM Checker";
    pub const LLM_CHECKER_DESCRIPTION: &str = "Runs optional LLM-based risk analysis.";
}

pub(crate) fn severity_zh(severity: RiskSeverity) -> &'static str {
    match severity {
        RiskSeverity::Critical => "严重",
        RiskSeverity::High => "高危",
        RiskSeverity::Medium => "中危",
        RiskSeverity::Low => "低危",
        RiskSeverity::Info => "信息",
    }
}

pub(crate) fn category_zh(category: RiskCategory) -> &'static str {
    match category {
        RiskCategory::PromptInjection => "提示词注入",
        RiskCategory::DataExfiltration => "数据外泄",
        RiskCategory::PrivilegeEscalation => "权限提升",
        RiskCategory::NetworkAccess => "网络访问",
        RiskCategory::FileSystem => "文件系统",
        RiskCategory::CredentialExposure => "凭据暴露",
        RiskCategory::SupplyChain => "供应链风险",
        RiskCategory::Misconfiguration => "配置错误",
        RiskCategory::Polyglot => "多语言混淆",
        RiskCategory::MaliciousExecution => "恶意执行",
        RiskCategory::CryptoMining => "加密货币挖矿",
        RiskCategory::WebShell => "WebShell",
        RiskCategory::HackTool => "攻击工具",
        RiskCategory::Exploit => "漏洞利用",
    }
}

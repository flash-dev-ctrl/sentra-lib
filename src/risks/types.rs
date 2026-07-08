use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

use crate::interfaces::{FileCategory, RiskCategory, RiskSeverity};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanOptions {
    pub checker: Option<CheckerConfig>,
    pub llm: Option<LlmConfig>,
    pub rules: Option<RuleDirectoryConfig>,
    pub online_ti: Option<OnlineTiConfig>,
    pub cache: Option<ScanCacheConfig>,
    pub concurrency: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanCacheConfig {
    pub path: Option<PathBuf>,
    #[serde(default, rename = "skipCache")]
    pub skip_cache: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckerConfig {
    pub enable_hash: Option<bool>,
    pub enable_llm: Option<bool>,
    pub enable_yara: Option<bool>,
    pub enable_local_ti: Option<bool>,
    pub enable_online_ti: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(rename = "apiUrl")]
    pub api_url: Option<String>,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub protocol: Option<crate::utils::protocol::WireProtocol>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<usize>,
    #[serde(rename = "maxPromptChars")]
    pub max_prompt_chars: Option<usize>,
    #[serde(rename = "timeoutMs")]
    pub timeout_ms: Option<u64>,
    pub stream: Option<bool>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleDirectoryConfig {
    pub yara: Option<PathBuf>,
    pub ti: Option<PathBuf>,
    pub hash: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleLoadSummary {
    pub yara: usize,
    pub ti_ips: usize,
    pub ti_domains: usize,
    pub hash_blacklist: usize,
    pub hash_whitelist: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    Yara,
    ThreatIntel,
    Hash,
}

impl RuleType {
    pub const ALL: [Self; 3] = [Self::Yara, Self::ThreatIntel, Self::Hash];
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct YaraRuleDef {
    pub name: String,
    pub source: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YaraRule {
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub meta: YaraRuleMeta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum YaraClassification {
    Benign,
    Suspicious,
    Harmful,
    Malicious,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YaraRuleMeta {
    pub author: String,
    pub title: String,
    pub title_zh: Option<String>,
    pub description: String,
    pub description_zh: Option<String>,
    pub remediation: Option<String>,
    pub remediation_zh: Option<String>,
    pub classification: YaraClassification,
    #[serde(rename = "threat_type")]
    pub threat_type: Option<String>,
    pub aitech: Option<String>,
    pub aisubtech: Option<String>,
    pub confidence: Option<String>,
    pub reference: Option<String>,
    pub severity: RiskSeverity,
    pub severity_zh: Option<String>,
    pub category: RiskCategory,
    pub category_zh: Option<String>,
    #[serde(rename = "file_type")]
    pub file_type: Option<String>,
    #[serde(default, rename = "file_categories")]
    pub file_categories: Vec<FileCategory>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TiRuleDef {
    pub malicious_ips: HashSet<String>,
    pub malicious_domains: HashSet<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HashRuleDef {
    pub blacklist: HashSet<String>,
    pub whitelist: HashSet<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnlineTiConfig {
    #[serde(rename = "cloudflareUrl")]
    pub cloudflare_url: Option<String>,
    #[serde(rename = "threatbookKey")]
    pub threatbook_key: Option<String>,
    #[serde(default, rename = "threatbookUrl")]
    pub threatbook_url: Option<String>,
    #[serde(rename = "chaitinKey")]
    pub chaitin_key: Option<String>,
    #[serde(default, rename = "chaitinUrl")]
    pub chaitin_url: Option<String>,
}

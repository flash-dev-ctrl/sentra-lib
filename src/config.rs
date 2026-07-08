use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::risks::types::CheckerConfig;
use crate::risks::{OnlineTiConfig, RuleDirectoryConfig};

pub const SENTRA_HOME_DIR_NAME: &str = ".sentra";

pub const SENTRA_CONFIG_FILE_NAME: &str = "config.json";

pub const SENTRA_HASH_RULE_DIR_NAME: &str = "hash";
pub const SENTRA_YARA_RULE_DIR_NAME: &str = "yara";
pub const SENTRA_TI_RULE_DIR_NAME: &str = "ti";
pub const SENTRA_CACHE_DIR_NAME: &str = "cache";
pub const SENTRA_SCAN_CACHE_FILE_NAME: &str = "scan-results.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SentraConfig {
    pub checker: Option<CheckerConfig>,
    pub llm: Option<LlmConfig>,
    pub rules: Option<RuleDirectoryConfig>,
    pub online_ti: Option<OnlineTiConfig>,
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
}

pub fn sentra_home(user_home: impl AsRef<Path>) -> PathBuf {
    user_home.as_ref().join(SENTRA_HOME_DIR_NAME)
}

pub fn sentra_config_file(user_home: impl AsRef<Path>) -> PathBuf {
    sentra_home(user_home).join(SENTRA_CONFIG_FILE_NAME)
}

pub fn sentra_hash_rule_dir(user_home: impl AsRef<Path>) -> PathBuf {
    sentra_home(user_home).join(SENTRA_HASH_RULE_DIR_NAME)
}

pub fn sentra_yara_rule_dir(user_home: impl AsRef<Path>) -> PathBuf {
    sentra_home(user_home).join(SENTRA_YARA_RULE_DIR_NAME)
}

pub fn sentra_ti_rule_dir(user_home: impl AsRef<Path>) -> PathBuf {
    sentra_home(user_home).join(SENTRA_TI_RULE_DIR_NAME)
}

pub fn sentra_cache_dir(user_home: impl AsRef<Path>) -> PathBuf {
    sentra_home(user_home).join(SENTRA_CACHE_DIR_NAME)
}

pub fn sentra_scan_cache_file(user_home: impl AsRef<Path>) -> PathBuf {
    sentra_cache_dir(user_home).join(SENTRA_SCAN_CACHE_FILE_NAME)
}

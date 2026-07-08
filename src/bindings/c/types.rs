use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::interfaces::AssetType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedAsset {
    pub user: String,
    pub agent: String,
    pub agent_title: String,
    pub agent_home: PathBuf,
    #[serde(rename = "type")]
    pub asset_type: AssetType,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerSelection {
    pub hash: Option<bool>,
    pub yara: Option<bool>,
    pub ti: Option<bool>,
    pub llm: Option<bool>,
    #[serde(rename = "onlineTi")]
    pub online_ti: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRequest {
    pub home: Option<PathBuf>,
    pub path: Option<PathBuf>,
    pub agents: Option<Vec<String>>,
    pub checkers: Option<ScannerSelection>,
}

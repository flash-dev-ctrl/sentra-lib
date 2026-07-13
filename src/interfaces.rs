use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use crate::SentraResult;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetType {
    Meta,
    #[default]
    Skill,
    Mcp,
    Memory,
    Cron,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Json,
    Yaml,
    Toml,
    Xml,
    Csv,
    Txt,
    Markdown,
    Sqlite,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileCategory {
    Unknown,
    Prompt,
    Script,
    Exe,
    Binary,
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileExtType {
    Unknown,
    Md,
    Json,
    Yaml,
    Js,
    Ts,
    Py,
    Sh,
    Ps1,
    Bat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskCategory {
    PromptInjection,
    DataExfiltration,
    PrivilegeEscalation,
    NetworkAccess,
    FileSystem,
    CredentialExposure,
    SupplyChain,
    Misconfiguration,
    Polyglot,
    MaliciousExecution,
    CryptoMining,
    WebShell,
    HackTool,
    Exploit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FindingLocation {
    pub line: usize,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub checker: String,
    pub severity: RiskSeverity,
    pub severity_zh: Option<String>,
    pub category: RiskCategory,
    pub category_zh: Option<String>,
    pub file: String,
    pub location: FindingLocation,
    pub title: String,
    pub title_zh: Option<String>,
    pub description: String,
    pub description_zh: Option<String>,
    pub evidence: Option<String>,
    pub context: Option<String>,
    pub remediation: String,
    pub remediation_zh: Option<String>,
}

impl Finding {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        checker: impl Into<String>,
        severity: RiskSeverity,
        category: RiskCategory,
        file: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            checker: checker.into(),
            severity,
            severity_zh: None,
            category,
            category_zh: None,
            file: file.into(),
            location: FindingLocation::default(),
            title: title.into(),
            title_zh: None,
            description: description.into(),
            description_zh: None,
            evidence: None,
            context: None,
            remediation: remediation.into(),
            remediation_zh: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentInput {
    pub source: String,
    pub content: String,
    pub file_cat: Option<FileCategory>,
    pub file_ext: Option<FileExtType>,
    #[serde(default)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolInput {
    pub source: String,
    pub tools: Vec<McpToolDef>,
    pub file_cat: Option<FileCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckInput {
    Content(ContentInput),
    McpTools(McpToolInput),
}

impl CheckInput {
    pub fn content(source: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Content(ContentInput {
            source: source.into(),
            content: content.into(),
            file_cat: None,
            file_ext: None,
            other: serde_json::Map::new(),
        })
    }

    pub fn with_file_meta(self, file_cat: FileCategory, file_ext: FileExtType) -> Self {
        match self {
            Self::Content(mut content) => {
                content.file_cat = Some(file_cat);
                content.file_ext = Some(file_ext);
                Self::Content(content)
            }
            Self::McpTools(mut input) => {
                input.file_cat = Some(file_cat);
                Self::McpTools(input)
            }
        }
    }

    pub fn with_hashes(self, hashes: crate::utils::Hashes) -> Self {
        match self {
            Self::Content(mut content) => {
                content.other.insert(
                    "hashes".to_string(),
                    serde_json::to_value(hashes).unwrap_or(serde_json::Value::Null),
                );
                Self::Content(content)
            }
            Self::McpTools(input) => Self::McpTools(input),
        }
    }

    pub fn source(&self) -> &str {
        match self {
            Self::Content(content) => &content.source,
            Self::McpTools(input) => &input.source,
        }
    }

    pub fn file_category(&self) -> Option<FileCategory> {
        match self {
            Self::Content(content) => content.file_cat,
            Self::McpTools(input) => input.file_cat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Skipped,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub checker: String,
    pub status: CheckStatus,
    pub reason: Option<String>,
    pub findings: Vec<Finding>,
}

pub trait Checker: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn categories(&self) -> &[FileCategory];
    fn check<'a>(
        &'a self,
        input: &'a CheckInput,
    ) -> Pin<Box<dyn Future<Output = SentraResult<CheckResult>> + Send + 'a>>;
}

pub trait Scanner<TAsset> {
    fn id(&self) -> &str;
    fn scan_asset<'a>(
        &'a self,
        asset: &'a TAsset,
    ) -> Pin<Box<dyn Future<Output = SentraResult<crate::risks::checkers::CheckOutput>> + Send + 'a>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MetaData {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub home: Option<PathBuf>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentInstallAction {
    Install,
    Update,
    Uninstall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstallResult {
    pub agent: String,
    pub action: AgentInstallAction,
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentInstallProgressStage {
    Trying,
    Verifying,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstallProgress {
    pub agent: String,
    pub action: AgentInstallAction,
    pub current: usize,
    pub total: usize,
    pub method: String,
    pub stage: AgentInstallProgressStage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillFileType {
    Config,
    Script,
    Prompt,
    Data,
    Documentation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillFile {
    pub path: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub file_type: SkillFileType,
    pub sha256: Option<String>,
    pub mtime: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SkillData {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub simhash: Option<String>,
    pub enabled: Option<bool>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub home: Option<PathBuf>,
    #[serde(default)]
    pub files: Vec<SkillFile>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct McpData {
    pub name: String,
    #[serde(rename = "type")]
    pub mcp_type: Option<McpType>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub url: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: Option<bool>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpType {
    Stdio,
    Sse,
    Http,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CronData {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub enabled: bool,
    pub home: Option<PathBuf>,
    #[serde(rename = "type")]
    pub cron_type: Option<CronType>,
    pub schedule: Option<String>,
    #[serde(default)]
    pub cwds: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<f64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<f64>,
    #[serde(default)]
    pub files: Vec<SkillFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CronType {
    At,
    Every,
    Cron,
    Rrule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProviderModel {
    pub id: String,
    pub name: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    #[default]
    Gateway,
    CodexAccount,
    ClaudeAccount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProviderAccount {
    #[serde(rename = "accountId")]
    pub account_id: Option<String>,
    pub email: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "authMode")]
    pub auth_mode: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "organizationId")]
    pub organization_id: Option<String>,
    #[serde(rename = "organizationName")]
    pub organization_name: Option<String>,
    #[serde(rename = "organizationRole")]
    pub organization_role: Option<String>,
    #[serde(rename = "organizationType")]
    pub organization_type: Option<String>,
    #[serde(rename = "billingType")]
    pub billing_type: Option<String>,
    pub plan: Option<String>,
    #[serde(rename = "hasExtraUsageEnabled")]
    pub has_extra_usage_enabled: Option<bool>,
    #[serde(rename = "accountCreatedAt")]
    pub account_created_at: Option<String>,
    #[serde(rename = "subscriptionCreatedAt")]
    pub subscription_created_at: Option<String>,
    #[serde(rename = "trialEndsAt")]
    pub trial_ends_at: Option<String>,
    #[serde(rename = "lastRefresh")]
    pub last_refresh: Option<String>,
    #[serde(rename = "profileFetchedAt")]
    pub profile_fetched_at: Option<serde_json::Value>,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<String>,
    #[serde(rename = "hasIdToken")]
    pub has_id_token: Option<bool>,
    #[serde(rename = "hasAccessToken")]
    pub has_access_token: Option<bool>,
    #[serde(rename = "hasRefreshToken")]
    pub has_refresh_token: Option<bool>,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProviderData {
    pub name: String,
    #[serde(rename = "providerType", default)]
    pub provider_type: ProviderType,
    #[serde(rename = "providerId", skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(
        rename = "providerDisplayName",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider_display_name: Option<String>,
    #[serde(rename = "rawProviderId", skip_serializing_if = "Option::is_none")]
    pub raw_provider_id: Option<String>,
    #[serde(rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(rename = "baseUrlSource", skip_serializing_if = "Option::is_none")]
    pub base_url_source: Option<crate::providers::ProviderFieldSource>,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    #[serde(rename = "activationStatus")]
    pub activation_status: crate::providers::ProviderActivationStatus,
    #[serde(default)]
    pub models: Vec<ProviderModel>,
    pub protocol: Option<crate::utils::protocol::WireProtocol>,
    #[serde(rename = "protocolSource", skip_serializing_if = "Option::is_none")]
    pub protocol_source: Option<crate::providers::ProviderFieldSource>,
    #[serde(rename = "endpointVariant", skip_serializing_if = "Option::is_none")]
    pub endpoint_variant: Option<String>,
    #[serde(default)]
    #[serde(rename = "resolutionStatus")]
    pub resolution_status: crate::providers::ProviderResolutionStatus,
    #[serde(rename = "resolutionReason", skip_serializing_if = "Option::is_none")]
    pub resolution_reason: Option<String>,
    #[serde(default)]
    #[serde(rename = "routeStatus")]
    pub route_status: crate::providers::ProviderRouteStatus,
    #[serde(rename = "routeReason", skip_serializing_if = "Option::is_none")]
    pub route_reason: Option<String>,
    #[serde(rename = "endpointTrust", skip_serializing_if = "Option::is_none")]
    pub endpoint_trust: Option<crate::providers::ProviderEndpointTrust>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<ProviderAccount>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MemoryData {
    pub name: String,
    pub size: u64,
    pub path: PathBuf,
    pub summary: Option<String>,
    pub format: Option<FileFormat>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProbeRequest {
    pub protocol: crate::utils::protocol::WireProtocol,
    pub body: Option<String>,
    #[serde(default)]
    pub prompt: Option<crate::utils::protocol::ModelPrompt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetMutationErrorCode {
    Unsupported,
    MissingRequiredField,
    MissingHome,
    PathOutsideAgentHome,
    NotFound,
    NotMatched,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetMutationError {
    pub code: AssetMutationErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetMutationResult {
    pub changed: bool,
    #[serde(default)]
    pub errors: Vec<AssetMutationError>,
}

impl AssetMutationResult {
    pub fn changed() -> Self {
        Self {
            changed: true,
            errors: Vec::new(),
        }
    }

    pub fn unchanged(code: AssetMutationErrorCode, message: impl Into<String>) -> Self {
        Self {
            changed: false,
            errors: vec![AssetMutationError {
                code,
                message: message.into(),
            }],
        }
    }
}

pub trait ErasedAsset {
    fn as_any(&self) -> &dyn Any;
    fn asset_type(&self) -> AssetType;
    fn agent_name(&self) -> &str;
    fn agent_home(&self) -> &Path;
    fn data(&self) -> SentraResult<serde_json::Value>;
    fn data_async<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = SentraResult<serde_json::Value>> + 'a>>;
    fn provider_requests(&self, _model: &str) -> Vec<ProviderProbeRequest> {
        Vec::new()
    }
    fn set_provider_data(&self, _value: ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "asset does not support set_provider_data",
        ))
    }
    fn del_provider_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "asset does not support del_provider_data",
        ))
    }
    fn set_skill_data(&self, _value: SkillData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "asset does not support set_skill_data",
        ))
    }
    fn del_skill_data(&self, _item: &SkillData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "asset does not support del_skill_data",
        ))
    }
}

pub trait Asset<TData, TItem = TData>: ErasedAsset {
    fn get_data(&self) -> SentraResult<TData>;

    fn get_data_async<'a>(&'a self) -> Pin<Box<dyn Future<Output = SentraResult<TData>> + 'a>> {
        Box::pin(async move { self.get_data() })
    }

    fn set_data(&self, _value: TItem) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "asset does not support set_data",
        ))
    }

    fn del_data(&self, _item: &TItem) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "asset does not support del_data",
        ))
    }
}

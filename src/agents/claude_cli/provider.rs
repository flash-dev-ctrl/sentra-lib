use serde_json::json;

use crate::SentraResult;
use crate::agents::install_status::hidden_home_parent;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderAccount, ProviderData,
    ProviderModel, ProviderProbeRequest, ProviderType,
};
use crate::utils::protocol::WireProtocol;
use crate::utils::{backup_file, mask_secret, read_json_file, write_json_file};

const COMPLETED_ONBOARDING_KEY: &str = "hasCompletedOnboarding";

#[derive(Debug, Clone)]
pub(super) struct ProviderAsset {
    pub(crate) core: AssetCore,
}

impl ProviderAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }

    pub fn get_request(&self, _model: &str) -> Vec<ProviderProbeRequest> {
        vec![ProviderProbeRequest {
            protocol: WireProtocol::AnthropicMessages,
            body: None,
            prompt: None,
        }]
    }
}

impl_erased_asset!(
    ProviderAsset,
    AssetType::Provider,
    Vec<ProviderData>,
    ProviderData,
    provider
);

impl Asset<Vec<ProviderData>, ProviderData> for ProviderAsset {
    fn get_data(&self) -> SentraResult<Vec<ProviderData>> {
        provider_data(self.core.agent_home(), true)
    }

    fn get_runtime_data(&self) -> SentraResult<Vec<ProviderData>> {
        provider_data(self.core.agent_home(), false)
    }

    fn set_data(&self, value: ProviderData) -> SentraResult<AssetMutationResult> {
        if value.provider_type != ProviderType::Gateway {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::Unsupported,
                "Claude Code account provider mutation is not supported",
            ));
        }
        let settings_path = self.core.agent_home().join("settings.json");
        let mut settings = read_json_file(&settings_path)?.unwrap_or_else(|| json!({}));
        if !settings.is_object() {
            settings = json!({});
        }
        if !settings.get("env").is_some_and(|value| value.is_object()) {
            settings["env"] = json!({});
        }
        if let Some(base_url) = value.base_url {
            settings["env"]["ANTHROPIC_BASE_URL"] = json!(base_url);
        }
        if let Some(api_key) = value.api_key {
            settings["env"]["ANTHROPIC_API_KEY"] = json!(api_key);
            if let Some(env) = settings
                .get_mut("env")
                .and_then(|value| value.as_object_mut())
            {
                env.remove("ANTHROPIC_AUTH_TOKEN");
            }
        }
        if let Some(model) = value.models.first() {
            for key in [
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
            ] {
                settings["env"][key] = json!(model.id);
            }
        }
        let (user_config_path, user_config) = optimized_user_config(self.core.agent_home())?;
        backup_file(&settings_path)?;
        backup_file(&user_config_path)?;
        write_json_file(settings_path, &settings)?;
        write_json_file(user_config_path, &user_config)?;
        Ok(AssetMutationResult::changed())
    }

    fn del_data(&self, item: &ProviderData) -> SentraResult<AssetMutationResult> {
        if item.provider_type != ProviderType::Gateway {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::Unsupported,
                "Claude Code account provider mutation is not supported",
            ));
        }
        let settings_path = self.core.agent_home().join("settings.json");
        let Some(mut settings) = read_json_file(&settings_path)? else {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                "settings.json does not exist",
            ));
        };
        let Some(env) = settings
            .get_mut("env")
            .and_then(|value| value.as_object_mut())
        else {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                "provider was not found in Claude Code settings",
            ));
        };
        let base_url = env
            .get("ANTHROPIC_BASE_URL")
            .and_then(|value| value.as_str());
        if base_url != item.base_url.as_deref() {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotMatched,
                "provider base URL did not match",
            ));
        }
        if let Some(api_key) = &item.api_key {
            let configured = env
                .get("ANTHROPIC_API_KEY")
                .or_else(|| env.get("ANTHROPIC_AUTH_TOKEN"))
                .and_then(|value| value.as_str());
            if configured != Some(api_key) {
                return Ok(AssetMutationResult::unchanged(
                    AssetMutationErrorCode::NotMatched,
                    "provider API key did not match",
                ));
            }
        }
        for key in [
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
        ] {
            env.remove(key);
        }
        backup_file(&settings_path)?;
        write_json_file(settings_path, &settings)?;
        Ok(AssetMutationResult::changed())
    }
}

fn optimized_user_config(
    agent_home: &std::path::Path,
) -> SentraResult<(std::path::PathBuf, serde_json::Value)> {
    let path = hidden_home_parent(agent_home).join(".claude.json");
    let mut config = match read_json_file(&path)? {
        Some(serde_json::Value::Object(config)) => config,
        Some(config) => serde_json::from_value(config)?,
        None => serde_json::Map::new(),
    };
    config.remove(COMPLETED_ONBOARDING_KEY);
    config.insert(COMPLETED_ONBOARDING_KEY.to_string(), json!(true));
    Ok((path, serde_json::Value::Object(config)))
}

fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let settings = read_json_file(agent_home.join("settings.json"))?.unwrap_or_else(|| json!({}));
    let env = settings.get("env").and_then(|value| value.as_object());
    let mut providers = Vec::new();
    if let Some(base_url) = env
        .and_then(|env| env.get("ANTHROPIC_BASE_URL"))
        .and_then(|value| value.as_str())
    {
        let api_key = env
            .and_then(|env| {
                env.get("ANTHROPIC_API_KEY")
                    .or_else(|| env.get("ANTHROPIC_AUTH_TOKEN"))
            })
            .and_then(|value| value.as_str())
            .and_then(|value| {
                if mask_secrets {
                    mask_secret(Some(value))
                } else {
                    Some(value.to_string())
                }
            });
        let mut seen = std::collections::HashSet::new();
        let mut models = Vec::new();
        for (id_key, name_key) in [
            ("ANTHROPIC_MODEL", None),
            (
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                Some("ANTHROPIC_DEFAULT_OPUS_MODEL_NAME"),
            ),
            (
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                Some("ANTHROPIC_DEFAULT_SONNET_MODEL_NAME"),
            ),
            (
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                Some("ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME"),
            ),
        ] {
            let Some(id) = env
                .and_then(|env| env.get(id_key))
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            if !seen.insert(id.to_string()) {
                continue;
            }
            let name = name_key
                .and_then(|key| env.and_then(|env| env.get(key)))
                .and_then(|value| value.as_str())
                .unwrap_or(id)
                .to_string();
            models.push(ProviderModel {
                id: id.to_string(),
                name: Some(name),
                enabled: true,
            });
        }
        providers.push(ProviderData {
            name: host_from_url(base_url).unwrap_or_else(|| "Anthropic".to_string()),
            base_url: Some(base_url.to_string()),
            api_key,
            enabled: true,
            models,
            protocol: None,
            ..ProviderData::default()
        });
    }
    if let Some(account) = credentials_account_provider(agent_home, env)? {
        providers.push(account);
    } else if let Some(account) = settings_oauth_account_provider(env) {
        providers.push(account);
    }
    Ok(providers)
}

fn credentials_account_provider(
    agent_home: &std::path::Path,
    settings_env: Option<&serde_json::Map<String, serde_json::Value>>,
) -> SentraResult<Option<ProviderData>> {
    let Some((credentials, source)) = claude_account_config(agent_home)? else {
        return Ok(None);
    };
    let oauth = credentials
        .get("oauthAccount")
        .and_then(|value| value.as_object());
    let has_settings_oauth = settings_env
        .and_then(|env| env.get("CLAUDE_CODE_OAUTH_TOKEN"))
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty());
    if oauth.is_none()
        && !has_token_like_key(&credentials, TokenKind::Access)
        && !has_token_like_key(&credentials, TokenKind::Refresh)
        && !has_settings_oauth
    {
        return Ok(None);
    }

    let mut metadata = serde_json::Map::new();
    for (source_key, target_key) in [
        ("organizationRateLimitTier", "organizationRateLimitTier"),
        ("userRateLimitTier", "userRateLimitTier"),
        ("seatTier", "seatTier"),
        ("workspaceRole", "workspaceRole"),
        ("claudeCodeTrialDurationDays", "trialDurationDays"),
        ("ccOnboardingFlags", "onboardingFlags"),
    ] {
        if let Some(value) = oauth
            .and_then(|oauth| oauth.get(source_key))
            .filter(|value| !value.is_null())
        {
            metadata.insert(target_key.to_string(), value.clone());
        }
    }
    let account = ProviderAccount {
        account_id: oauth.and_then(|oauth| json_string(oauth, "accountUuid")),
        email: oauth.and_then(|oauth| json_string(oauth, "emailAddress")),
        display_name: oauth.and_then(|oauth| json_string(oauth, "displayName")),
        auth_mode: Some("oauth".to_string()),
        source: Some(source),
        organization_id: oauth.and_then(|oauth| json_string(oauth, "organizationUuid")),
        organization_name: oauth.and_then(|oauth| json_string(oauth, "organizationName")),
        organization_role: oauth.and_then(|oauth| json_string(oauth, "organizationRole")),
        organization_type: oauth.and_then(|oauth| json_string(oauth, "organizationType")),
        billing_type: oauth.and_then(|oauth| json_string(oauth, "billingType")),
        plan: oauth.and_then(|oauth| {
            json_string(oauth, "organizationType").or_else(|| json_string(oauth, "billingType"))
        }),
        has_extra_usage_enabled: oauth
            .and_then(|oauth| oauth.get("hasExtraUsageEnabled"))
            .and_then(|value| value.as_bool()),
        account_created_at: oauth.and_then(|oauth| json_string(oauth, "accountCreatedAt")),
        subscription_created_at: oauth
            .and_then(|oauth| json_string(oauth, "subscriptionCreatedAt")),
        trial_ends_at: oauth.and_then(|oauth| json_string(oauth, "claudeCodeTrialEndsAt")),
        last_refresh: None,
        profile_fetched_at: oauth
            .and_then(|oauth| oauth.get("profileFetchedAt"))
            .filter(|value| !value.is_null())
            .cloned(),
        expires_at: credentials_expires_at(&credentials),
        has_id_token: Some(has_token_like_key(&credentials, TokenKind::Id)),
        has_access_token: Some(
            has_token_like_key(&credentials, TokenKind::Access) || has_settings_oauth,
        ),
        has_refresh_token: Some(has_token_like_key(&credentials, TokenKind::Refresh)),
        metadata,
    };
    Ok(Some(account_provider_from_account(account)))
}

fn claude_account_config(
    agent_home: &std::path::Path,
) -> SentraResult<Option<(serde_json::Value, String)>> {
    if let Some(user_home) = agent_home.parent() {
        let path = user_home.join(".claude.json");
        if let Some(config) = read_json_file(&path)? {
            return Ok(Some((config, ".claude.json".to_string())));
        }
    }
    let path = agent_home.join(".credentials.json");
    Ok(read_json_file(path)?.map(|config| (config, ".credentials.json".to_string())))
}

fn settings_oauth_account_provider(
    env: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<ProviderData> {
    let has_oauth_token = env
        .and_then(|env| env.get("CLAUDE_CODE_OAUTH_TOKEN"))
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty());
    if !has_oauth_token {
        return None;
    }
    let account = ProviderAccount {
        auth_mode: Some("oauth".to_string()),
        source: Some("settings.json".to_string()),
        has_id_token: Some(false),
        has_access_token: Some(true),
        has_refresh_token: Some(false),
        ..ProviderAccount::default()
    };
    Some(account_provider_from_account(account))
}

fn account_provider_from_account(account: ProviderAccount) -> ProviderData {
    let name = account
        .display_name
        .clone()
        .or_else(|| account.email.clone())
        .or_else(|| account.organization_name.clone())
        .or_else(|| account.account_id.clone())
        .unwrap_or_else(|| "Claude Code Account".to_string());
    ProviderData {
        name,
        provider_type: ProviderType::ClaudeAccount,
        base_url: None,
        api_key: None,
        enabled: true,
        models: Vec::new(),
        protocol: None,
        account: Some(account),
        ..ProviderData::default()
    }
}

fn json_string(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn credentials_expires_at(credentials: &serde_json::Value) -> Option<String> {
    find_token_value(credentials, "expires_at")
        .or_else(|| find_token_value(credentials, "expiresAt"))
        .or_else(|| find_token_value(credentials, "expiry"))
}

#[derive(Debug, Clone, Copy)]
enum TokenKind {
    Id,
    Access,
    Refresh,
}

fn has_token_like_key(value: &serde_json::Value, kind: TokenKind) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, value)| {
            (token_key_matches(key, kind)
                && value.as_str().is_some_and(|value| !value.trim().is_empty()))
                || has_token_like_key(value, kind)
        }),
        serde_json::Value::Array(items) => {
            items.iter().any(|value| has_token_like_key(value, kind))
        }
        _ => false,
    }
}

fn token_key_matches(key: &str, kind: TokenKind) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    match kind {
        TokenKind::Id => normalized.contains("idtoken"),
        TokenKind::Access => normalized.contains("accesstoken"),
        TokenKind::Refresh => normalized.contains("refreshtoken"),
    }
}

fn find_token_value(value: &serde_json::Value, key: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => map
            .get(key)
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .or_else(|| map.values().find_map(|value| find_token_value(value, key))),
        serde_json::Value::Array(items) => {
            items.iter().find_map(|value| find_token_value(value, key))
        }
        _ => None,
    }
}

fn host_from_url(value: &str) -> Option<String> {
    let rest = value.split_once("://")?.1;
    rest.split(['/', '?', '#', ':'])
        .next()
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn set_data_backs_up_and_optimizes_user_config_for_cli_and_ide() {
        for agent_name in ["claude-cli", "claude-cli-ide"] {
            let user_home = tempfile::tempdir().unwrap();
            let agent_home = user_home.path().join(".claude");
            fs::create_dir_all(&agent_home).unwrap();
            fs::write(
                agent_home.join("settings.json"),
                r#"{"env":{"KEEP":"value"}}"#,
            )
            .unwrap();
            let original_user_config =
                r#"{"theme":"dark","hasCompletedOnboarding":false,"locale":"en-US"}"#;
            fs::write(user_home.path().join(".claude.json"), original_user_config).unwrap();

            let result = ProviderAsset::new(agent_name, &agent_home)
                .set_data(gateway_provider())
                .unwrap();

            assert!(result.changed);
            let user_config = read_json_file(user_home.path().join(".claude.json"))
                .unwrap()
                .expect("optimized user config");
            assert_eq!(user_config["theme"], "dark");
            assert_eq!(user_config["locale"], "en-US");
            assert_eq!(user_config[COMPLETED_ONBOARDING_KEY], true);
            assert_eq!(
                user_config
                    .as_object()
                    .unwrap()
                    .keys()
                    .next_back()
                    .map(String::as_str),
                Some(COMPLETED_ONBOARDING_KEY)
            );
            let settings = read_json_file(agent_home.join("settings.json"))
                .unwrap()
                .expect("updated model settings");
            assert_eq!(settings["env"]["KEEP"], "value");
            assert_eq!(settings["env"]["ANTHROPIC_MODEL"], "claude-test");

            let backups = backup_paths(user_home.path(), ".claude.json.bak.");
            assert_eq!(backups.len(), 1);
            assert_eq!(
                fs::read_to_string(&backups[0]).unwrap(),
                original_user_config
            );
        }
    }

    #[test]
    fn set_data_creates_optimized_user_config_when_missing() {
        let user_home = tempfile::tempdir().unwrap();
        let agent_home = user_home.path().join(".claude");
        fs::create_dir_all(&agent_home).unwrap();

        ProviderAsset::new("claude-cli", &agent_home)
            .set_data(gateway_provider())
            .unwrap();

        let user_config = read_json_file(user_home.path().join(".claude.json"))
            .unwrap()
            .expect("created user config");
        assert_eq!(user_config[COMPLETED_ONBOARDING_KEY], true);
        assert!(backup_paths(user_home.path(), ".claude.json.bak.").is_empty());
    }

    #[test]
    fn invalid_or_non_object_user_config_stops_before_model_settings_are_written() {
        for original_user_config in ["{invalid", "[]"] {
            let user_home = tempfile::tempdir().unwrap();
            let agent_home = user_home.path().join(".claude");
            fs::create_dir_all(&agent_home).unwrap();
            let original_settings = r#"{"env":{"KEEP":"value"}}"#;
            fs::write(agent_home.join("settings.json"), original_settings).unwrap();
            fs::write(user_home.path().join(".claude.json"), original_user_config).unwrap();

            let result =
                ProviderAsset::new("claude-cli-ide", &agent_home).set_data(gateway_provider());

            assert!(result.is_err());
            assert_eq!(
                fs::read_to_string(agent_home.join("settings.json")).unwrap(),
                original_settings
            );
            assert!(backup_paths(&agent_home, "settings.json.bak.").is_empty());
            assert!(backup_paths(user_home.path(), ".claude.json.bak.").is_empty());
        }
    }

    fn gateway_provider() -> ProviderData {
        ProviderData {
            name: "Gateway".to_string(),
            provider_type: ProviderType::Gateway,
            base_url: Some("https://gateway.example.test".to_string()),
            api_key: Some("sk-test".to_string()),
            enabled: true,
            models: vec![ProviderModel {
                id: "claude-test".to_string(),
                name: Some("Claude Test".to_string()),
                enabled: true,
            }],
            ..ProviderData::default()
        }
    }

    fn backup_paths(dir: &Path, prefix: &str) -> Vec<PathBuf> {
        let mut paths = fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(prefix))
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }
}

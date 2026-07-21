use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
};
use crate::utils::protocol::WireProtocol;
use crate::utils::{mask_secret, read_json_file, read_text_file};

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

    pub fn get_request(&self, _model: &str) -> Vec<crate::interfaces::ProviderProbeRequest> {
        Vec::new()
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

    fn set_data(&self, _value: ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Hermes provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Hermes provider mutation is not supported",
        ))
    }
}

fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let json = read_yaml_file(&agent_home.join("config.yaml"))?
        .and_then(|config| serde_json::to_value(config).ok())
        .unwrap_or(Value::Null);
    let auth = read_json_file(agent_home.join("auth.json"))?.unwrap_or(Value::Null);
    let dotenv = read_dotenv(agent_home.join(".env"))?;
    let active_provider = configured_provider(&json)
        .filter(|provider| provider != "auto")
        .or_else(|| {
            auth.get("active_provider")
                .and_then(Value::as_str)
                .map(normalized_provider_id)
        });
    let mut results = Vec::new();
    let mut fallback_models = BTreeMap::<String, Vec<String>>::new();

    for (key, value) in json.as_object().into_iter().flatten() {
        let Some(prefix) = key.strip_suffix("_providers") else {
            continue;
        };
        let Some(items) = value.as_array() else {
            continue;
        };
        for (index, item) in items.iter().enumerate() {
            let Some(raw) = item.as_object() else {
                continue;
            };
            let provider_ref = raw
                .get("provider")
                .and_then(Value::as_str)
                .map(normalized_provider_id);
            let model_ref = raw
                .get("model")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            if !raw.contains_key("base_url")
                && !raw.contains_key("api_key")
                && !raw.contains_key("name")
                && let (Some(provider_ref), Some(model_ref)) = (&provider_ref, &model_ref)
            {
                fallback_models
                    .entry(provider_ref.clone())
                    .or_default()
                    .push(model_ref.clone());
                continue;
            }
            add_provider(
                &mut results,
                make_provider(
                    provider_ref.as_deref(),
                    raw.get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{prefix}-{index}")),
                    raw.get("base_url")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    raw.get("api_key")
                        .and_then(Value::as_str)
                        .and_then(|value| maybe_mask_secret(value, mask_secrets))
                        .or_else(|| {
                            provider_ref.as_deref().and_then(|provider| {
                                configured_provider_secret(&auth, &dotenv, provider, mask_secrets)
                            })
                        }),
                    model_list(raw.get("models")),
                    provider_enabled(active_provider.as_deref(), provider_ref.as_deref()),
                    raw_protocol(raw),
                ),
            );
        }
    }

    for (name, models) in fallback_models {
        add_provider(
            &mut results,
            make_provider(
                Some(&name),
                name.clone(),
                None,
                configured_provider_secret(&auth, &dotenv, &name, mask_secrets),
                models
                    .into_iter()
                    .map(|id| ProviderModel {
                        name: Some(id.clone()),
                        id,
                        enabled: true,
                    })
                    .collect(),
                provider_enabled(active_provider.as_deref(), Some(&name)),
                None,
            ),
        );
    }

    if let Some(raw) = json.get("model").and_then(|value| value.as_object()) {
        let provider_id = raw
            .get("provider")
            .and_then(Value::as_str)
            .map(normalized_provider_id)
            .or_else(|| {
                raw.get("default")
                    .and_then(Value::as_str)
                    .and_then(provider_from_model_ref)
            });
        let model_id = raw
            .get("default")
            .or_else(|| raw.get("name"))
            .and_then(Value::as_str)
            .map(model_id_from_ref);
        if let Some(provider_id) = provider_id {
            add_provider(
                &mut results,
                make_provider(
                    Some(&provider_id),
                    provider_id.clone(),
                    raw.get("base_url")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    raw.get("api_key")
                        .and_then(Value::as_str)
                        .and_then(|value| maybe_mask_secret(value, mask_secrets))
                        .or_else(|| {
                            configured_provider_secret(&auth, &dotenv, &provider_id, mask_secrets)
                        }),
                    model_id
                        .map(|id| vec![provider_model(&id)])
                        .unwrap_or_default(),
                    true,
                    raw_protocol(raw),
                ),
            );
        }
    } else if let Some(model_ref) = json.get("model").and_then(Value::as_str)
        && let Some(provider_id) = provider_from_model_ref(model_ref)
    {
        let model_id = model_id_from_ref(model_ref);
        add_provider(
            &mut results,
            make_provider(
                Some(&provider_id),
                provider_id.clone(),
                None,
                configured_provider_secret(&auth, &dotenv, &provider_id, mask_secrets),
                vec![provider_model(&model_id)],
                true,
                None,
            ),
        );
    }

    let mut discovered_provider_ids = auth_provider_ids(&auth);
    discovered_provider_ids.extend(env_provider_ids(&dotenv));
    if let Some(active_provider) = active_provider.as_ref() {
        discovered_provider_ids.insert(active_provider.clone());
    }
    for provider_id in discovered_provider_ids {
        add_provider(
            &mut results,
            make_provider(
                Some(&provider_id),
                provider_id.clone(),
                None,
                configured_provider_secret(&auth, &dotenv, &provider_id, mask_secrets),
                Vec::new(),
                provider_enabled(active_provider.as_deref(), Some(&provider_id)),
                None,
            ),
        );
    }

    Ok(results)
}

fn make_provider(
    provider_id: Option<&str>,
    name: String,
    base_url: Option<String>,
    api_key: Option<String>,
    models: Vec<ProviderModel>,
    enabled: bool,
    protocol: Option<WireProtocol>,
) -> ProviderData {
    let provider_id = provider_id.map(normalized_provider_id);
    ProviderData {
        name,
        provider_id: provider_id.clone(),
        raw_provider_id: provider_id.clone(),
        base_url: base_url.or_else(|| {
            provider_id
                .as_deref()
                .and_then(known_provider_base_url)
                .map(str::to_string)
        }),
        api_key,
        enabled,
        models: models.clone(),
        protocol: protocol.or_else(|| {
            provider_id
                .as_deref()
                .and_then(|id| inferred_protocol(id, &models))
        }),
        ..ProviderData::default()
    }
}

fn configured_provider(config: &Value) -> Option<String> {
    let model = config.get("model")?;
    if let Some(raw) = model.as_object() {
        return raw
            .get("provider")
            .and_then(Value::as_str)
            .map(normalized_provider_id)
            .or_else(|| {
                raw.get("default")
                    .and_then(Value::as_str)
                    .and_then(provider_from_model_ref)
            });
    }
    model.as_str().and_then(provider_from_model_ref)
}

fn normalized_provider_id(value: &str) -> String {
    value
        .trim()
        .strip_prefix("custom:")
        .unwrap_or(value.trim())
        .to_ascii_lowercase()
}

fn provider_from_model_ref(value: &str) -> Option<String> {
    let (provider, _) = value.split_once('/')?;
    (!provider.trim().is_empty()).then(|| normalized_provider_id(provider))
}

fn model_id_from_ref(value: &str) -> String {
    value
        .split_once('/')
        .map(|(_, model)| model)
        .unwrap_or(value)
        .to_string()
}

fn provider_model(id: &str) -> ProviderModel {
    ProviderModel {
        id: id.to_string(),
        name: Some(id.to_string()),
        enabled: true,
    }
}

fn provider_enabled(active_provider: Option<&str>, provider_id: Option<&str>) -> bool {
    match (active_provider, provider_id) {
        (Some(active), Some(provider)) => active == provider,
        _ => true,
    }
}

fn raw_protocol(raw: &serde_json::Map<String, Value>) -> Option<WireProtocol> {
    raw.get("api")
        .or_else(|| raw.get("protocol"))
        .or_else(|| raw.get("transport"))
        .and_then(Value::as_str)
        .and_then(protocol_for_api)
}

fn protocol_for_api(value: &str) -> Option<WireProtocol> {
    match value {
        "openai-responses"
        | "openai-codex-responses"
        | "azure-openai-responses"
        | "codex-responses"
        | "codex_responses" => Some(WireProtocol::Responses),
        "openai-completions" | "openai-chat-completions" | "openai-chat" | "openai_chat" => {
            Some(WireProtocol::ChatCompletions)
        }
        "anthropic" | "anthropic-messages" | "anthropic_messages" => {
            Some(WireProtocol::AnthropicMessages)
        }
        _ => value.parse().ok(),
    }
}

fn inferred_protocol(provider_id: &str, models: &[ProviderModel]) -> Option<WireProtocol> {
    match provider_id {
        "anthropic" | "minimax" | "minimax-cn" => Some(WireProtocol::AnthropicMessages),
        "openai-codex" => Some(WireProtocol::Responses),
        "opencode-go" if !models.is_empty() => {
            let anthropic = models.iter().all(|model| {
                let id = model.id.to_ascii_lowercase();
                id.starts_with("minimax-") || id.starts_with("qwen3.7-")
            });
            Some(if anthropic {
                WireProtocol::AnthropicMessages
            } else {
                WireProtocol::ChatCompletions
            })
        }
        "deepseek" | "kimi" | "openai" | "opencode" | "openrouter" => {
            Some(WireProtocol::ChatCompletions)
        }
        _ => None,
    }
}

fn known_provider_base_url(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "anthropic" => Some("https://api.anthropic.com"),
        "deepseek" => Some("https://api.deepseek.com"),
        "kimi" => Some("https://api.moonshot.cn/v1"),
        "minimax" => Some("https://api.minimax.io/anthropic"),
        "minimax-cn" => Some("https://api.minimaxi.com/anthropic"),
        "openai" => Some("https://api.openai.com/v1"),
        "opencode" => Some("https://opencode.ai/zen/v1"),
        "opencode-go" => Some("https://opencode.ai/zen/go/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        _ => None,
    }
}

fn auth_provider_ids(auth: &Value) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for key in ["providers", "credential_pool"] {
        if let Some(providers) = auth.get(key).and_then(Value::as_object) {
            ids.extend(providers.keys().map(|id| normalized_provider_id(id)));
        }
    }
    ids
}

fn env_provider_ids(dotenv: &BTreeMap<String, String>) -> BTreeSet<String> {
    dotenv
        .keys()
        .filter_map(|key| provider_id_from_env_key(key))
        .collect()
}

fn provider_id_from_env_key(key: &str) -> Option<String> {
    let key = key.strip_suffix("_API_KEY")?;
    Some(match key {
        "GEMINI" | "GOOGLE" => "google".to_string(),
        "MOONSHOT" => "kimi".to_string(),
        other => other.to_ascii_lowercase().replace('_', "-"),
    })
}

fn configured_provider_secret(
    auth: &Value,
    dotenv: &BTreeMap<String, String>,
    provider_id: &str,
    mask_secrets: bool,
) -> Option<String> {
    ["providers", "credential_pool"]
        .iter()
        .find_map(|key| auth.get(*key).and_then(|items| items.get(provider_id)))
        .and_then(find_secret_value)
        .and_then(|value| maybe_mask_secret(value, mask_secrets))
        .or_else(|| environment_secret(dotenv, provider_id, mask_secrets))
}

fn find_secret_value(value: &Value) -> Option<&str> {
    const SECRET_FIELDS: &[&str] = &[
        "api_key",
        "apiKey",
        "key",
        "token",
        "access_token",
        "accessToken",
        "refresh_token",
        "refreshToken",
    ];
    match value {
        Value::Object(raw) => SECRET_FIELDS
            .iter()
            .find_map(|key| raw.get(*key).and_then(Value::as_str))
            .or_else(|| raw.values().find_map(find_secret_value)),
        Value::Array(items) => items.iter().find_map(find_secret_value),
        _ => None,
    }
}

fn environment_secret(
    dotenv: &BTreeMap<String, String>,
    provider_id: &str,
    mask_secrets: bool,
) -> Option<String> {
    let generic = format!(
        "{}_API_KEY",
        provider_id.replace('-', "_").to_ascii_uppercase()
    );
    let known: &[&str] = match provider_id {
        "google" => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        "kimi" => &["MOONSHOT_API_KEY", "KIMI_API_KEY"],
        "minimax" | "minimax-cn" => &["MINIMAX_API_KEY"],
        _ => &[],
    };
    known
        .iter()
        .copied()
        .chain(std::iter::once(generic.as_str()))
        .find_map(|key| environment_value(dotenv, key))
        .and_then(|value| maybe_mask_secret(&value, mask_secrets))
}

fn maybe_mask_secret(value: &str, mask_secrets: bool) -> Option<String> {
    if mask_secrets {
        mask_secret(Some(value))
    } else {
        Some(value.to_string())
    }
}

fn model_list(raw: Option<&serde_json::Value>) -> Vec<ProviderModel> {
    let Some(map) = raw.and_then(|value| value.as_object()) else {
        return Vec::new();
    };
    map.iter()
        .map(|(id, value)| ProviderModel {
            id: id.clone(),
            name: value
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .or_else(|| Some(id.clone())),
            enabled: true,
        })
        .collect()
}

fn add_provider(results: &mut Vec<ProviderData>, provider: ProviderData) {
    let Some(existing) = results.iter_mut().find(|item| {
        (item.raw_provider_id.is_some() && item.raw_provider_id == provider.raw_provider_id)
            || (item.raw_provider_id.is_none()
                && provider.raw_provider_id.is_none()
                && item.name == provider.name)
    }) else {
        results.push(provider);
        return;
    };
    let mut seen = existing
        .models
        .iter()
        .map(|model| model.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for model in provider.models {
        if seen.insert(model.id.clone()) {
            existing.models.push(model);
        }
    }
    if existing.base_url.is_none() {
        existing.base_url = provider.base_url;
    }
    if existing.api_key.is_none() {
        existing.api_key = provider.api_key;
    }
    existing.enabled |= provider.enabled;
}

fn read_yaml_file(path: &std::path::Path) -> SentraResult<Option<serde_yaml::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    serde_yaml::from_str(&content).map(Some).map_err(Into::into)
}

fn read_dotenv(path: std::path::PathBuf) -> SentraResult<BTreeMap<String, String>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(BTreeMap::new());
    };
    let mut values = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let mut chars = key.chars();
        if !chars
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
            || !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            continue;
        }
        let value = value.trim();
        let value = if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            &value[1..value.len() - 1]
        } else {
            value
        };
        if !value.is_empty() {
            values.insert(key.to_string(), value.to_string());
        }
    }
    Ok(values)
}

fn environment_value(dotenv: &BTreeMap<String, String>, key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| dotenv.get(key).cloned())
}

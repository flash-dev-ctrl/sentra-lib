use std::collections::BTreeMap;

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest,
};
use crate::providers::{
    ProviderActivationStatus, ProviderCandidate, ProviderFieldSource, ProviderRegistry,
    protocol_for_api,
};
use crate::utils::protocol::{WireProtocol, default_model_probe_prompt};
use crate::utils::{mask_secret, read_json_file};

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
        [
            WireProtocol::Responses,
            WireProtocol::ChatCompletions,
            WireProtocol::AnthropicMessages,
        ]
        .into_iter()
        .map(|protocol| ProviderProbeRequest {
            protocol,
            body: None,
            prompt: Some(default_model_probe_prompt()),
        })
        .collect()
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
        provider_data(self.core.agent_home())
    }

    fn set_data(&self, _value: ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "OpenCode provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "OpenCode provider mutation is not supported",
        ))
    }
}

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let auth_keys = auth_api_keys(agent_home)?;
    let mut results = Vec::new();
    let mut provider_ids = Vec::new();

    for path in crate::agents::opencode::config_files(agent_home) {
        let Some(config) = read_json_file(path)? else {
            continue;
        };
        let active_model = config.get("model").and_then(Value::as_str);
        let active_model_ref = active_model.and_then(split_model_ref);
        let Some(providers) = config
            .get("provider")
            .or_else(|| config.get("providers"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (provider_id, raw) in providers {
            if provider_ids.iter().any(|id| id == provider_id) {
                continue;
            }
            let raw = raw.as_object();
            let options = raw
                .and_then(|raw| raw.get("options"))
                .and_then(Value::as_object);
            let mut models = provider_models(raw.and_then(|raw| raw.get("models")));
            if let Some((active_provider, active_model)) = active_model_ref
                && active_provider == provider_id
            {
                ensure_model(&mut models, active_model, true);
            }

            let explicit_enabled = raw
                .and_then(|raw| raw.get("enabled"))
                .and_then(Value::as_bool);
            let configured_protocol = raw
                .and_then(|raw| {
                    raw.get("api")
                        .or_else(|| raw.get("protocol"))
                        .and_then(Value::as_str)
                })
                .or_else(|| {
                    options.and_then(|options| {
                        options
                            .get("api")
                            .or_else(|| options.get("protocol"))
                            .and_then(Value::as_str)
                    })
                })
                .and_then(parse_protocol);
            let inferred_protocol =
                configured_protocol.or_else(|| npm_protocol(raw.and_then(|raw| raw.get("npm"))));

            let mut candidate = ProviderCandidate::new("opencode");
            candidate.agent_provider_id = Some(provider_id.clone());
            candidate.display_name = Some(
                raw.and_then(|raw| {
                    raw.get("name")
                        .or_else(|| raw.get("displayName"))
                        .and_then(Value::as_str)
                })
                .unwrap_or(provider_id)
                .to_string(),
            );
            candidate.configured_base_url = base_url(raw, options);
            candidate.protocol_hint = inferred_protocol;
            candidate.protocol_source = inferred_protocol.map(|_| {
                if configured_protocol.is_some() {
                    ProviderFieldSource::Configured
                } else {
                    ProviderFieldSource::Inferred
                }
            });
            candidate.api_key = configured_api_key(raw, options)
                .or_else(|| auth_keys.get(provider_id).cloned())
                .or_else(|| environment_api_key(provider_id));
            candidate.activation = provider_activation(
                active_model_ref.map(|(provider, _)| provider),
                provider_id,
                explicit_enabled,
            );
            candidate.models = models;
            results.push(ProviderRegistry::builtin().resolve(candidate));
            provider_ids.push(provider_id.clone());
        }
    }

    for (provider_id, api_key) in auth_keys {
        if provider_ids.iter().any(|id| id == &provider_id) {
            continue;
        }
        let mut candidate = ProviderCandidate::new("opencode");
        candidate.agent_provider_id = Some(provider_id.clone());
        candidate.display_name = Some(provider_id.clone());
        candidate.api_key = Some(api_key);
        candidate.activation = ProviderActivationStatus::Unknown;
        results.push(ProviderRegistry::builtin().resolve(candidate));
    }

    Ok(results)
}

fn provider_models(raw: Option<&Value>) -> Vec<ProviderModel> {
    match raw {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(id) => Some(model(id, Some(id), true)),
                Value::Object(raw) => {
                    let id = raw
                        .get("id")
                        .or_else(|| raw.get("name"))
                        .and_then(Value::as_str)?;
                    Some(model(
                        id,
                        raw.get("displayName")
                            .or_else(|| raw.get("label"))
                            .or_else(|| raw.get("name"))
                            .and_then(Value::as_str),
                        raw.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                    ))
                }
                _ => None,
            })
            .collect(),
        Some(Value::Object(items)) => items
            .iter()
            .map(|(id, raw)| {
                model(
                    id,
                    raw.get("name")
                        .or_else(|| raw.get("displayName"))
                        .or_else(|| raw.get("label"))
                        .and_then(Value::as_str)
                        .or(Some(id)),
                    raw.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn ensure_model(models: &mut Vec<ProviderModel>, id: &str, enabled: bool) {
    if let Some(model) = models.iter_mut().find(|model| model.id == id) {
        model.enabled = enabled;
        return;
    }
    models.push(model(id, Some(id), enabled));
}

fn model(id: &str, name: Option<&str>, enabled: bool) -> ProviderModel {
    ProviderModel {
        id: id.to_string(),
        name: name.map(str::to_string).or_else(|| Some(id.to_string())),
        enabled,
    }
}

fn split_model_ref(value: &str) -> Option<(&str, &str)> {
    let (provider, model) = value.split_once('/')?;
    (!provider.is_empty() && !model.is_empty()).then_some((provider, model))
}

fn base_url(
    raw: Option<&serde_json::Map<String, Value>>,
    options: Option<&serde_json::Map<String, Value>>,
) -> Option<String> {
    string_field(options, &["baseURL", "baseUrl", "base_url", "url"])
        .or_else(|| string_field(raw, &["baseURL", "baseUrl", "base_url", "url"]))
}

fn configured_api_key(
    raw: Option<&serde_json::Map<String, Value>>,
    options: Option<&serde_json::Map<String, Value>>,
) -> Option<String> {
    string_field(options, &["apiKey", "api_key", "key", "token"])
        .or_else(|| string_field(raw, &["apiKey", "api_key", "key", "token"]))
        .and_then(|value| resolve_secret(&value))
        .and_then(|value| mask_secret(Some(&value)))
}

fn string_field(raw: Option<&serde_json::Map<String, Value>>, keys: &[&str]) -> Option<String> {
    let raw = raw?;
    keys.iter()
        .find_map(|key| raw.get(*key).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn parse_protocol(value: &str) -> Option<WireProtocol> {
    protocol_for_api(value).or_else(|| value.parse().ok())
}

fn npm_protocol(raw: Option<&Value>) -> Option<WireProtocol> {
    let value = raw.and_then(Value::as_str)?.to_ascii_lowercase();
    if value.contains("anthropic") {
        Some(WireProtocol::AnthropicMessages)
    } else if value.contains("openai-compatible") || value.contains("openai") {
        Some(WireProtocol::ChatCompletions)
    } else {
        None
    }
}

fn provider_activation(
    active_provider: Option<&str>,
    provider_id: &str,
    explicit_enabled: Option<bool>,
) -> ProviderActivationStatus {
    match active_provider {
        Some(active) if active == provider_id => ProviderActivationStatus::Active,
        Some(_) => ProviderActivationStatus::Inactive,
        None => match explicit_enabled {
            Some(true) => ProviderActivationStatus::Active,
            Some(false) => ProviderActivationStatus::Inactive,
            None => ProviderActivationStatus::Unknown,
        },
    }
}

fn auth_api_keys(agent_home: &std::path::Path) -> SentraResult<BTreeMap<String, String>> {
    let Some(auth) =
        read_json_file(crate::agents::opencode::data_home(agent_home).join("auth.json"))?
    else {
        return Ok(BTreeMap::new());
    };
    let mut keys = BTreeMap::new();
    for container in ["provider", "providers", "credential", "credentials"] {
        if let Some(map) = auth.get(container).and_then(Value::as_object) {
            collect_provider_secret_map(map, &mut keys);
        }
    }
    if let Some(map) = auth.as_object() {
        for (provider_id, value) in map {
            if value.is_object()
                && let Some(secret) = secret_from_value(value)
            {
                keys.insert(provider_id.clone(), secret);
            }
        }
    }
    Ok(keys)
}

fn collect_provider_secret_map(
    map: &serde_json::Map<String, Value>,
    keys: &mut BTreeMap<String, String>,
) {
    for (provider_id, value) in map {
        if let Some(secret) = secret_from_value(value) {
            keys.insert(provider_id.clone(), secret);
        }
    }
}

fn secret_from_value(value: &Value) -> Option<String> {
    let raw = if let Some(value) = value.as_str() {
        Some(value)
    } else {
        let value = value.as_object()?;
        [
            "apiKey",
            "api_key",
            "key",
            "token",
            "access",
            "accessToken",
            "access_token",
            "value",
        ]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
    }?;
    resolve_secret(raw).and_then(|value| mask_secret(Some(&value)))
}

fn resolve_secret(value: &str) -> Option<String> {
    if value.trim().is_empty() || value.starts_with('!') {
        return None;
    }
    if let Some(name) = value
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
    {
        return std::env::var(name).ok().filter(|value| !value.is_empty());
    }
    if let Some(name) = value.strip_prefix('$')
        && !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return std::env::var(name).ok().filter(|value| !value.is_empty());
    }
    Some(value.to_string())
}

fn environment_api_key(provider_id: &str) -> Option<String> {
    ProviderRegistry::builtin()
        .environment_keys("opencode", provider_id)
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
        .and_then(|value| mask_secret(Some(&value)))
}

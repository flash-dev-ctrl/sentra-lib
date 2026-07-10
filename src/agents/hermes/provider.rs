use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
};
use crate::providers::{
    ProviderActivationStatus, ProviderCandidate, ProviderFieldSource, ProviderRegistry,
    protocol_for_api,
};
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
        provider_data(self.core.agent_home())
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

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let json = read_yaml_file(&agent_home.join("config.yaml"))?
        .and_then(|config| serde_json::to_value(config).ok())
        .unwrap_or(Value::Null);
    let auth = read_json_file(agent_home.join("auth.json"))?.unwrap_or(Value::Null);
    let dotenv = read_dotenv(agent_home.join(".env"))?;
    let configured_provider = configured_provider(&json);
    let auth_active_provider = auth
        .get("active_provider")
        .and_then(Value::as_str)
        .map(normalized_provider_id);
    let active_provider = configured_provider
        .as_deref()
        .filter(|provider| *provider != "auto")
        .map(str::to_string)
        .or(auth_active_provider);
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
            let model_ref = raw.get("model").and_then(Value::as_str).map(str::to_string);
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
            let protocol = raw_protocol(raw);
            add_provider(
                &mut results,
                resolve_provider(
                    provider_ref.as_deref(),
                    raw.get("name")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{prefix}-{index}")),
                    raw.get("base_url")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    mask_secret(raw.get("api_key").and_then(Value::as_str)),
                    model_list(raw.get("models")),
                    activation_for_provider(active_provider.as_deref(), provider_ref.as_deref()),
                    protocol,
                    protocol.map(|_| ProviderFieldSource::Configured),
                ),
            );
        }
    }

    for (name, models) in fallback_models {
        let models = models
            .into_iter()
            .map(|id| ProviderModel {
                name: Some(id.clone()),
                id,
                enabled: true,
            })
            .collect::<Vec<_>>();
        let protocol = protocol_for_provider_models(&name, &models);
        add_provider(
            &mut results,
            resolve_provider(
                Some(&name),
                name.clone(),
                None,
                None,
                models,
                activation_for_provider(active_provider.as_deref(), Some(&name)),
                protocol,
                protocol.map(|_| ProviderFieldSource::Inferred),
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
        if let Some(provider_id) = provider_id {
            let id = raw
                .get("default")
                .or_else(|| raw.get("name"))
                .and_then(Value::as_str)
                .map(model_id_from_ref);
            let api_key = mask_secret(raw.get("api_key").and_then(Value::as_str))
                .or_else(|| configured_provider_secret(&auth, &dotenv, &provider_id));
            let configured_protocol = raw_protocol(raw);
            let protocol = configured_protocol.or_else(|| {
                id.as_deref()
                    .and_then(|id| protocol_for_hermes_model(&provider_id, id))
            });
            add_provider(
                &mut results,
                resolve_provider(
                    Some(&provider_id),
                    provider_id.clone(),
                    raw.get("base_url")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    api_key,
                    id.map(|id| vec![provider_model(&id)]).unwrap_or_default(),
                    ProviderActivationStatus::Active,
                    protocol,
                    protocol.map(|_| {
                        if configured_protocol.is_some() {
                            ProviderFieldSource::Configured
                        } else {
                            ProviderFieldSource::Inferred
                        }
                    }),
                ),
            );
        }
    } else if let Some(model_ref) = json.get("model").and_then(Value::as_str)
        && let Some(provider_id) = provider_from_model_ref(model_ref)
    {
        let model_id = model_id_from_ref(model_ref);
        let protocol = protocol_for_hermes_model(&provider_id, &model_id);
        add_provider(
            &mut results,
            resolve_provider(
                Some(&provider_id),
                provider_id.clone(),
                None,
                configured_provider_secret(&auth, &dotenv, &provider_id),
                vec![provider_model(&model_id)],
                ProviderActivationStatus::Active,
                protocol,
                protocol.map(|_| ProviderFieldSource::Inferred),
            ),
        );
    }

    let mut discovered_provider_ids = auth_provider_ids(&auth);
    discovered_provider_ids.extend(env_provider_ids(&dotenv));
    if let Some(active_provider) = active_provider.as_ref() {
        discovered_provider_ids.insert(active_provider.clone());
    }
    for provider_id in discovered_provider_ids {
        let activation = activation_for_provider(active_provider.as_deref(), Some(&provider_id));
        let provider = resolve_provider(
            Some(&provider_id),
            provider_id.clone(),
            None,
            configured_provider_secret(&auth, &dotenv, &provider_id),
            Vec::new(),
            activation,
            None,
            None,
        );
        add_provider(&mut results, provider);
    }

    Ok(results)
}

fn resolve_provider(
    provider_id: Option<&str>,
    name: String,
    base_url: Option<String>,
    api_key: Option<String>,
    models: Vec<ProviderModel>,
    activation: ProviderActivationStatus,
    protocol: Option<crate::utils::protocol::WireProtocol>,
    protocol_source: Option<ProviderFieldSource>,
) -> ProviderData {
    let mut candidate = ProviderCandidate::new("hermes");
    candidate.agent_provider_id = provider_id.map(str::to_string);
    candidate.display_name = Some(name);
    candidate.configured_base_url = base_url;
    candidate.api_key = api_key;
    candidate.activation = activation;
    candidate.models = models;
    candidate.protocol_hint = protocol;
    candidate.protocol_source = protocol_source;
    let mut provider = ProviderRegistry::builtin().resolve(candidate);
    provider.enabled = activation != ProviderActivationStatus::Inactive;
    provider
}

fn configured_provider(config: &Value) -> Option<String> {
    let model = config.get("model")?;
    if let Some(raw) = model.as_object() {
        return raw
            .get("provider")
            .and_then(Value::as_str)
            .map(normalized_provider_id);
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

fn activation_for_provider(
    active_provider: Option<&str>,
    provider_id: Option<&str>,
) -> ProviderActivationStatus {
    match (active_provider, provider_id) {
        (Some(active), Some(provider)) if active == provider => ProviderActivationStatus::Active,
        (Some(_), Some(_)) => ProviderActivationStatus::Inactive,
        _ => ProviderActivationStatus::Unknown,
    }
}

fn raw_protocol(
    raw: &serde_json::Map<String, Value>,
) -> Option<crate::utils::protocol::WireProtocol> {
    raw.get("api")
        .or_else(|| raw.get("protocol"))
        .or_else(|| raw.get("transport"))
        .and_then(Value::as_str)
        .and_then(protocol_for_api)
}

fn protocol_for_hermes_model(
    provider_id: &str,
    model_id: &str,
) -> Option<crate::utils::protocol::WireProtocol> {
    if provider_id != "opencode-go" {
        return None;
    }
    let id = model_id.to_ascii_lowercase();
    Some(
        if id.starts_with("minimax-") || id.starts_with("qwen3.7-") {
            crate::utils::protocol::WireProtocol::AnthropicMessages
        } else {
            crate::utils::protocol::WireProtocol::ChatCompletions
        },
    )
}

fn protocol_for_provider_models(
    provider_id: &str,
    models: &[ProviderModel],
) -> Option<crate::utils::protocol::WireProtocol> {
    let mut protocols = models
        .iter()
        .filter_map(|model| protocol_for_hermes_model(provider_id, &model.id));
    let first = protocols.next()?;
    protocols.all(|protocol| protocol == first).then_some(first)
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
    let registry = ProviderRegistry::builtin();
    let mut ids = BTreeSet::new();
    for provider in registry.providers() {
        if registry
            .environment_keys("hermes", &provider.id)
            .iter()
            .any(|key| environment_value(dotenv, key).is_some())
        {
            ids.insert(provider.id.clone());
        }
        for alias in provider
            .agent_aliases
            .iter()
            .filter(|alias| alias.agent == "hermes")
        {
            if registry
                .environment_keys("hermes", &alias.alias)
                .iter()
                .any(|key| environment_value(dotenv, key).is_some())
            {
                ids.insert(alias.alias.clone());
            }
        }
    }
    ids
}

fn configured_provider_secret(
    auth: &Value,
    dotenv: &BTreeMap<String, String>,
    provider_id: &str,
) -> Option<String> {
    let auth_value = ["providers", "credential_pool"]
        .iter()
        .find_map(|key| auth.get(*key).and_then(|items| items.get(provider_id)))
        .and_then(find_secret_value)
        .and_then(|value| mask_secret(Some(value)));
    auth_value.or_else(|| {
        ProviderRegistry::builtin()
            .environment_keys("hermes", provider_id)
            .into_iter()
            .find_map(|key| environment_value(dotenv, key))
            .and_then(|value| mask_secret(Some(&value)))
    })
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
        let mut key_chars = key.chars();
        let valid_key = key_chars
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
            && key_chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric());
        if !valid_key {
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
        item.raw_provider_id == provider.raw_provider_id
            || (item.raw_provider_id.is_none()
                && provider.raw_provider_id.is_none()
                && item.provider_id.is_some()
                && item.provider_id == provider.provider_id)
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
        existing.base_url_source = provider.base_url_source;
        existing.endpoint_variant = provider.endpoint_variant;
    }
    if existing.protocol.is_none() {
        existing.protocol = provider.protocol;
        existing.protocol_source = provider.protocol_source;
    }
    if provider.activation_status == ProviderActivationStatus::Active {
        existing.activation_status = ProviderActivationStatus::Active;
        existing.enabled = true;
    } else if provider.activation_status == ProviderActivationStatus::Inactive
        && existing.activation_status == ProviderActivationStatus::Unknown
    {
        existing.activation_status = ProviderActivationStatus::Inactive;
        existing.enabled = false;
    }
    if existing.api_key.is_none() {
        existing.api_key = provider.api_key;
    }
}

fn read_yaml_file(path: &std::path::Path) -> SentraResult<Option<serde_yaml::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    serde_yaml::from_str(&content).map(Some).map_err(Into::into)
}

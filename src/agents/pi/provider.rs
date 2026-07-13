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
            "Pi provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Pi provider mutation is not supported",
        ))
    }
}

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let settings = read_json_file(agent_home.join("settings.json"))?.unwrap_or(Value::Null);
    let models_config = read_json_file(agent_home.join("models.json"))?.unwrap_or(Value::Null);
    let auth = read_json_file(agent_home.join("auth.json"))?.unwrap_or(Value::Null);

    let default_provider = string_at(&settings, &["defaultProvider"]);
    let default_model = string_at(&settings, &["defaultModel"]);
    let auth_providers = auth_providers(&auth);
    let mut providers = Vec::new();
    let mut provider_ids = Vec::new();

    if let Some(raw_providers) = models_config
        .get("providers")
        .or_else(|| models_config.get("modelProviders"))
        .and_then(Value::as_object)
    {
        for (id, raw) in raw_providers {
            let raw = raw.as_object();
            let provider_id = id.as_str();
            let api = raw.and_then(|raw| raw.get("api").and_then(Value::as_str));
            let activation = provider_activation(default_provider.as_deref(), provider_id);
            let mut models = provider_models(raw.and_then(|raw| raw.get("models")));
            if activation == ProviderActivationStatus::Active
                && let Some(model) = default_model.as_deref()
            {
                ensure_model(&mut models, model, true);
            }

            let mut candidate = ProviderCandidate::new("pi");
            candidate.agent_provider_id = Some(provider_id.to_string());
            candidate.display_name = Some(
                raw.and_then(|raw| {
                    raw.get("name")
                        .or_else(|| raw.get("displayName"))
                        .and_then(Value::as_str)
                })
                .unwrap_or(provider_id)
                .to_string(),
            );
            candidate.configured_base_url = raw.and_then(base_url);
            candidate.protocol_hint = api.and_then(protocol_for_api);
            candidate.protocol_source = candidate
                .protocol_hint
                .map(|_| ProviderFieldSource::Configured);
            candidate.api_key = raw
                .and_then(|raw| {
                    raw.get("apiKey")
                        .or_else(|| raw.get("api_key"))
                        .and_then(Value::as_str)
                        .and_then(|value| resolve_config_string(value, None))
                })
                .or_else(|| auth_key(auth_providers, provider_id))
                .or_else(|| env_api_key(provider_id))
                .and_then(|value| mask_secret(Some(&value)));
            candidate.activation = activation;
            candidate.models = models;
            providers.push(ProviderRegistry::builtin().resolve(candidate));
            provider_ids.push(provider_id.to_string());
        }

        if let Some(provider_id) = default_provider.as_deref()
            && !contains_provider_id(&provider_ids, provider_id)
        {
            providers.push(fallback_provider(
                provider_id,
                default_model.as_deref(),
                auth_providers,
                ProviderActivationStatus::Active,
            ));
            provider_ids.push(provider_id.to_string());
        }
    }

    if let Some(provider_id) = default_provider.as_deref()
        && !contains_provider_id(&provider_ids, provider_id)
    {
        providers.push(fallback_provider(
            provider_id,
            default_model.as_deref(),
            auth_providers,
            ProviderActivationStatus::Active,
        ));
        provider_ids.push(provider_id.to_string());
    }

    if let Some(auth_providers) = auth_providers {
        for provider_id in auth_providers.keys() {
            if contains_provider_id(&provider_ids, provider_id) {
                continue;
            }
            providers.push(fallback_provider(
                provider_id,
                None,
                Some(auth_providers),
                provider_activation(default_provider.as_deref(), provider_id),
            ));
            provider_ids.push(provider_id.to_string());
        }
    }

    Ok(providers)
}

fn contains_provider_id(provider_ids: &[String], provider_id: &str) -> bool {
    provider_ids.iter().any(|id| id == provider_id)
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
            .map(|(id, value)| {
                let name = value
                    .get("name")
                    .or_else(|| value.get("displayName"))
                    .or_else(|| value.get("label"))
                    .and_then(Value::as_str)
                    .unwrap_or(id);
                model(id, Some(name), true)
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

fn fallback_provider(
    provider_id: &str,
    default_model: Option<&str>,
    auth_providers: Option<&serde_json::Map<String, Value>>,
    activation: ProviderActivationStatus,
) -> ProviderData {
    let mut candidate = ProviderCandidate::new("pi");
    candidate.agent_provider_id = Some(provider_id.to_string());
    candidate.display_name = Some(provider_id.to_string());
    candidate.api_key = auth_key(auth_providers, provider_id)
        .or_else(|| env_api_key(provider_id))
        .and_then(|value| mask_secret(Some(&value)));
    candidate.activation = activation;
    candidate.models = default_model
        .map(|id| vec![model(id, Some(id), true)])
        .unwrap_or_default();
    ProviderRegistry::builtin().resolve(candidate)
}

fn provider_activation(
    default_provider: Option<&str>,
    provider_id: &str,
) -> ProviderActivationStatus {
    match default_provider {
        Some(default_provider) if default_provider == provider_id => {
            ProviderActivationStatus::Active
        }
        Some(_) => ProviderActivationStatus::Inactive,
        None => ProviderActivationStatus::Unknown,
    }
}

fn base_url(raw: &serde_json::Map<String, Value>) -> Option<String> {
    raw.get("baseURL")
        .or_else(|| raw.get("baseUrl"))
        .or_else(|| raw.get("base_url"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn string_at(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn auth_providers(auth: &Value) -> Option<&serde_json::Map<String, Value>> {
    auth.get("providers")
        .and_then(Value::as_object)
        .or_else(|| auth.as_object())
}

fn auth_key(
    providers: Option<&serde_json::Map<String, Value>>,
    provider_id: &str,
) -> Option<String> {
    let value = providers?.get(provider_id)?;
    if let Some(key) = value.as_str() {
        return resolve_config_string(key, None);
    }
    let raw = value.as_object()?;
    let scoped_env = raw.get("env").and_then(Value::as_object);
    raw.get("apiKey")
        .or_else(|| raw.get("api_key"))
        .or_else(|| raw.get("key"))
        .or_else(|| raw.get("token"))
        .and_then(Value::as_str)
        .and_then(|value| resolve_config_string(value, scoped_env))
}

fn resolve_config_string(
    value: &str,
    scoped_env: Option<&serde_json::Map<String, Value>>,
) -> Option<String> {
    if value.starts_with('!') {
        return None;
    }

    let mut resolved = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            resolved.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('$') => {
                chars.next();
                resolved.push('$');
            }
            Some('!') => {
                chars.next();
                resolved.push('!');
            }
            Some('{') => {
                chars.next();
                let mut name = String::new();
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                    name.push(next);
                }
                resolved.push_str(&env_value(&name, scoped_env)?);
            }
            Some(next) if is_env_start(next) => {
                let mut name = String::new();
                while let Some(next) = chars.peek().copied() {
                    if !is_env_part(next) {
                        break;
                    }
                    name.push(next);
                    chars.next();
                }
                resolved.push_str(&env_value(&name, scoped_env)?);
            }
            _ => resolved.push('$'),
        }
    }
    Some(resolved)
}

fn env_value(name: &str, scoped_env: Option<&serde_json::Map<String, Value>>) -> Option<String> {
    scoped_env
        .and_then(|env| env.get(name))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| std::env::var(name).ok())
}

fn is_env_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_env_part(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn env_api_key(provider_id: &str) -> Option<String> {
    for key in ProviderRegistry::builtin().environment_keys("pi", provider_id) {
        if let Ok(value) = std::env::var(key)
            && !value.is_empty()
        {
            return Some(value);
        }
    }
    None
}

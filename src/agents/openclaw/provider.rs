use std::collections::BTreeMap;

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
            "OpenClaw provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "OpenClaw provider mutation is not supported",
        ))
    }
}

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let Some(config) = read_json_file(agent_home.join("openclaw.json"))? else {
        return Ok(Vec::new());
    };
    let selections = collect_model_selections(&config);
    let providers = config
        .get("models")
        .and_then(|models| models.get("providers"))
        .or_else(|| config.get("providers"))
        .or_else(|| config.get("modelProviders"));
    let entries: Vec<(String, &Value)> = if let Some(items) = providers.and_then(Value::as_array) {
        items
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let id = value
                    .get("id")
                    .or_else(|| value.get("provider"))
                    .or_else(|| value.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| index.to_string());
                (id, value)
            })
            .collect()
    } else if let Some(map) = providers.and_then(Value::as_object) {
        map.iter()
            .map(|(key, value)| (key.clone(), value))
            .collect()
    } else {
        Vec::new()
    };

    let mut results = Vec::new();
    for (provider_id, value) in entries {
        let raw = value.as_object();
        let explicit_enabled = raw
            .and_then(|raw| raw.get("enabled"))
            .and_then(Value::as_bool);
        let selection = selections.get(&provider_id);
        let activation = provider_activation(selection, explicit_enabled);
        let mut models = provider_models(raw.and_then(|raw| raw.get("models")));
        merge_models(
            &mut models,
            selection.into_iter().flat_map(|item| item.models.values()),
        );

        let configured_protocol = raw
            .and_then(|raw| {
                raw.get("api")
                    .or_else(|| raw.get("protocol"))
                    .and_then(Value::as_str)
            })
            .and_then(protocol_for_api);
        let inferred_protocol =
            configured_protocol.or_else(|| protocol_for_selected_models(&provider_id, &models));

        let mut candidate = ProviderCandidate::new("openclaw");
        candidate.agent_provider_id = Some(provider_id.clone());
        candidate.display_name = raw
            .and_then(|raw| {
                raw.get("name")
                    .or_else(|| raw.get("displayName"))
                    .and_then(Value::as_str)
            })
            .map(str::to_string)
            .or_else(|| Some(provider_id.clone()));
        candidate.configured_base_url = raw
            .and_then(|raw| {
                raw.get("baseUrl")
                    .or_else(|| raw.get("base_url"))
                    .and_then(Value::as_str)
            })
            .map(str::to_string);
        candidate.protocol_hint = inferred_protocol;
        candidate.protocol_source = inferred_protocol.map(|_| {
            if configured_protocol.is_some() {
                ProviderFieldSource::Configured
            } else {
                ProviderFieldSource::Inferred
            }
        });
        candidate.api_key = mask_secret(raw.and_then(|raw| {
            raw.get("apiKey")
                .or_else(|| raw.get("api_key"))
                .and_then(Value::as_str)
        }))
        .or_else(|| environment_api_key(&provider_id));
        candidate.activation = activation;
        candidate.models = models;
        let mut provider = ProviderRegistry::builtin().resolve(candidate);
        provider.enabled = match activation {
            ProviderActivationStatus::Active => true,
            ProviderActivationStatus::Inactive => false,
            ProviderActivationStatus::Unknown => explicit_enabled.unwrap_or(true),
        };
        merge_provider(&mut results, provider);
    }

    for (provider_id, selection) in selections {
        if results.iter().any(|provider: &ProviderData| {
            provider.raw_provider_id.as_deref() == Some(&provider_id)
        }) {
            continue;
        }
        let models = selection.models.into_values().collect::<Vec<_>>();
        let mut candidate = ProviderCandidate::new("openclaw");
        candidate.agent_provider_id = Some(provider_id.clone());
        candidate.display_name = Some(provider_id.clone());
        candidate.protocol_hint = protocol_for_selected_models(&provider_id, &models);
        candidate.protocol_source = candidate
            .protocol_hint
            .map(|_| ProviderFieldSource::Inferred);
        candidate.api_key = environment_api_key(&provider_id);
        candidate.activation = if selection.active {
            ProviderActivationStatus::Active
        } else {
            ProviderActivationStatus::Inactive
        };
        candidate.models = models;
        merge_provider(&mut results, ProviderRegistry::builtin().resolve(candidate));
    }

    Ok(results)
}

#[derive(Default)]
struct ProviderSelection {
    active: bool,
    models: BTreeMap<String, ProviderModel>,
}

fn collect_model_selections(config: &Value) -> BTreeMap<String, ProviderSelection> {
    let mut selections = BTreeMap::new();
    if let Some(defaults) = config.get("agents").and_then(|value| value.get("defaults")) {
        collect_agent_models(defaults, &mut selections);
    }
    if let Some(agents) = config
        .get("agents")
        .and_then(|value| value.get("list"))
        .and_then(Value::as_array)
    {
        for agent in agents {
            collect_agent_models(agent, &mut selections);
        }
    }
    if let Some(model) = config.get("model") {
        collect_model_choice(model, &mut selections);
    }
    selections
}

fn collect_agent_models(value: &Value, selections: &mut BTreeMap<String, ProviderSelection>) {
    if let Some(model) = value.get("model") {
        collect_model_choice(model, selections);
    }
    if let Some(models) = value.get("models").and_then(Value::as_object) {
        for (model_ref, raw) in models {
            let name = raw
                .get("alias")
                .or_else(|| raw.get("name"))
                .and_then(Value::as_str);
            add_model_ref(selections, model_ref, name, false);
        }
    }
}

fn collect_model_choice(value: &Value, selections: &mut BTreeMap<String, ProviderSelection>) {
    if let Some(primary) = value.as_str() {
        add_model_ref(selections, primary, None, true);
        return;
    }
    let Some(raw) = value.as_object() else {
        return;
    };
    if let Some(primary) = raw.get("primary").and_then(Value::as_str) {
        add_model_ref(selections, primary, None, true);
    }
    if let Some(fallbacks) = raw.get("fallbacks").and_then(Value::as_array) {
        for fallback in fallbacks.iter().filter_map(Value::as_str) {
            add_model_ref(selections, fallback, None, false);
        }
    }
}

fn add_model_ref(
    selections: &mut BTreeMap<String, ProviderSelection>,
    model_ref: &str,
    name: Option<&str>,
    active: bool,
) {
    let Some((provider_id, model_id)) = model_ref.split_once('/') else {
        return;
    };
    let provider_id = provider_id.trim();
    let model_id = model_id.trim();
    if provider_id.is_empty() || model_id.is_empty() {
        return;
    }
    let selection = selections.entry(provider_id.to_string()).or_default();
    selection.active |= active;
    if model_id != "*" {
        selection.models.insert(
            model_id.to_string(),
            ProviderModel {
                id: model_id.to_string(),
                name: Some(name.unwrap_or(model_id).to_string()),
                enabled: true,
            },
        );
    }
}

fn provider_activation(
    selection: Option<&ProviderSelection>,
    explicit_enabled: Option<bool>,
) -> ProviderActivationStatus {
    if let Some(selection) = selection {
        return if selection.active {
            ProviderActivationStatus::Active
        } else {
            ProviderActivationStatus::Inactive
        };
    }
    match explicit_enabled {
        Some(true) => ProviderActivationStatus::Active,
        Some(false) => ProviderActivationStatus::Inactive,
        None => ProviderActivationStatus::Unknown,
    }
}

fn provider_models(raw: Option<&Value>) -> Vec<ProviderModel> {
    match raw {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|value| match value {
                Value::String(id) => Some(provider_model(id, Some(id))),
                Value::Object(item) => {
                    let id = item
                        .get("id")
                        .or_else(|| item.get("name"))
                        .and_then(Value::as_str)?;
                    Some(provider_model(id, item.get("name").and_then(Value::as_str)))
                }
                _ => None,
            })
            .collect(),
        Some(Value::Object(items)) => items
            .iter()
            .map(|(id, raw)| provider_model(id, raw.get("name").and_then(Value::as_str)))
            .collect(),
        _ => Vec::new(),
    }
}

fn provider_model(id: &str, name: Option<&str>) -> ProviderModel {
    ProviderModel {
        id: id.to_string(),
        name: Some(name.unwrap_or(id).to_string()),
        enabled: true,
    }
}

fn merge_models<'a>(
    models: &mut Vec<ProviderModel>,
    additions: impl Iterator<Item = &'a ProviderModel>,
) {
    for model in additions {
        if !models.iter().any(|existing| existing.id == model.id) {
            models.push(model.clone());
        }
    }
}

fn protocol_for_selected_models(
    provider_id: &str,
    models: &[ProviderModel],
) -> Option<crate::utils::protocol::WireProtocol> {
    if provider_id != "opencode-go" || models.is_empty() {
        return None;
    }
    let protocols = models
        .iter()
        .map(|model| {
            let id = model.id.to_ascii_lowercase();
            if id.starts_with("minimax-") || id.starts_with("qwen3.7-") {
                crate::utils::protocol::WireProtocol::AnthropicMessages
            } else {
                crate::utils::protocol::WireProtocol::ChatCompletions
            }
        })
        .collect::<std::collections::HashSet<_>>();
    (protocols.len() == 1).then(|| *protocols.iter().next().unwrap())
}

fn environment_api_key(provider_id: &str) -> Option<String> {
    ProviderRegistry::builtin()
        .environment_keys("openclaw", provider_id)
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
        .and_then(|value| mask_secret(Some(&value)))
}

fn merge_provider(results: &mut Vec<ProviderData>, provider: ProviderData) {
    let existing = results.iter_mut().find(|existing| {
        existing.raw_provider_id == provider.raw_provider_id
            || (existing.raw_provider_id.is_none()
                && provider.raw_provider_id.is_none()
                && existing.provider_id.is_some()
                && existing.provider_id == provider.provider_id)
    });
    let Some(existing) = existing else {
        results.push(provider);
        return;
    };
    merge_models(&mut existing.models, provider.models.iter());
    if provider.activation_status == ProviderActivationStatus::Active {
        existing.activation_status = ProviderActivationStatus::Active;
        existing.enabled = true;
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
    if existing.api_key.is_none() {
        existing.api_key = provider.api_key;
    }
}

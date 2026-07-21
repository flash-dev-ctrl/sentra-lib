use std::collections::BTreeMap;

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
};
use crate::utils::protocol::WireProtocol;
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
        provider_data(self.core.agent_home(), true)
    }

    fn get_runtime_data(&self) -> SentraResult<Vec<ProviderData>> {
        provider_data(self.core.agent_home(), false)
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

fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let Some(config) = read_json_file(agent_home.join("openclaw.json"))? else {
        return Ok(Vec::new());
    };
    let mut selections = collect_model_selections(&config);
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
        let selection = selections.remove(&provider_id);
        let mut models = provider_models(raw.and_then(|raw| raw.get("models")));
        if let Some(selection) = selection.as_ref() {
            merge_models(&mut models, selection.models.values());
        }
        let configured_protocol = raw
            .and_then(|raw| raw.get("api").or_else(|| raw.get("protocol")))
            .and_then(Value::as_str)
            .and_then(protocol_for_api);
        results.push(ProviderData {
            name: raw
                .and_then(|raw| {
                    raw.get("name")
                        .or_else(|| raw.get("displayName"))
                        .and_then(Value::as_str)
                })
                .unwrap_or(&provider_id)
                .to_string(),
            provider_id: Some(provider_id.clone()),
            raw_provider_id: Some(provider_id.clone()),
            base_url: raw
                .and_then(|raw| {
                    raw.get("baseUrl")
                        .or_else(|| raw.get("base_url"))
                        .and_then(Value::as_str)
                })
                .map(str::to_string),
            api_key: raw
                .and_then(|raw| {
                    raw.get("apiKey")
                        .or_else(|| raw.get("api_key"))
                        .and_then(Value::as_str)
                })
                .and_then(|value| maybe_mask_secret(value, mask_secrets)),
            enabled: selection
                .as_ref()
                .map(|selection| selection.active)
                .unwrap_or_else(|| {
                    raw.and_then(|raw| raw.get("enabled"))
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                }),
            protocol: configured_protocol.or_else(|| inferred_protocol(&provider_id, &models)),
            models,
            ..ProviderData::default()
        });
    }

    for (provider_id, selection) in selections {
        let models = selection.models.into_values().collect::<Vec<_>>();
        results.push(ProviderData {
            name: provider_id.clone(),
            provider_id: Some(provider_id.clone()),
            raw_provider_id: Some(provider_id.clone()),
            base_url: known_provider_base_url(&provider_id).map(str::to_string),
            api_key: None,
            enabled: selection.active,
            protocol: inferred_protocol(&provider_id, &models),
            models,
            ..ProviderData::default()
        });
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
        "deepseek" | "openai" | "opencode" => Some(WireProtocol::ChatCompletions),
        _ => None,
    }
}

fn known_provider_base_url(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "deepseek" => Some("https://api.deepseek.com"),
        "minimax" => Some("https://api.minimax.io/anthropic"),
        "minimax-cn" => Some("https://api.minimaxi.com/anthropic"),
        "opencode" => Some("https://opencode.ai/zen/v1"),
        "opencode-go" => Some("https://opencode.ai/zen/go/v1"),
        _ => None,
    }
}

fn maybe_mask_secret(value: &str, mask_secrets: bool) -> Option<String> {
    if mask_secrets {
        mask_secret(Some(value))
    } else {
        Some(value.to_string())
    }
}

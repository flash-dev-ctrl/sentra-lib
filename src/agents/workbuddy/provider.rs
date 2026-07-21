use std::collections::BTreeMap;

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest,
};
use crate::utils::protocol::{WireProtocol, build_model_probe_request};
use crate::utils::{mask_secret, read_json_file};

#[derive(Debug, Clone)]
pub(super) struct ProviderAsset {
    core: AssetCore,
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

    pub fn get_request(&self, model: &str) -> Vec<ProviderProbeRequest> {
        [WireProtocol::Responses, WireProtocol::ChatCompletions]
            .into_iter()
            .map(|protocol| {
                let request = build_model_probe_request(protocol, model);
                ProviderProbeRequest {
                    protocol: request.protocol,
                    body: request.body,
                    prompt: None,
                }
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
        provider_data(self.core.agent_home(), true)
    }

    fn get_runtime_data(&self) -> SentraResult<Vec<ProviderData>> {
        provider_data(self.core.agent_home(), false)
    }

    fn set_data(&self, _value: ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "WorkBuddy provider mutation is not supported",
        ))
    }
}

fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let Some(config) = read_json_file(agent_home.join("models.json"))? else {
        return Ok(Vec::new());
    };
    Ok(parse_providers(&config, mask_secrets))
}

fn parse_providers(raw: &Value, mask_secrets: bool) -> Vec<ProviderData> {
    match raw {
        Value::Array(items) => parse_model_entries(items, mask_secrets),
        Value::Object(map) => parse_provider_map(
            map.get("providers")
                .or_else(|| map.get("modelProviders"))
                .unwrap_or(raw),
            mask_secrets,
        ),
        _ => Vec::new(),
    }
}

fn parse_model_entries(items: &[Value], mask_secrets: bool) -> Vec<ProviderData> {
    let mut grouped = BTreeMap::new();

    for item in items.iter().filter_map(Value::as_object) {
        let Some(model_id) = string(Some(item), "id") else {
            continue;
        };
        let model_name = string(Some(item), "name").unwrap_or_else(|| model_id.clone());
        let base_url = string(Some(item), "url")
            .or_else(|| string(Some(item), "baseURL"))
            .or_else(|| string(Some(item), "baseUrl"))
            .or_else(|| string(Some(item), "base_url"));
        let raw_api_key = string(Some(item), "apiKey").or_else(|| string(Some(item), "api_key"));
        let provider_name = string(Some(item), "vendor")
            .or_else(|| base_url.as_deref().and_then(host_from_url))
            .unwrap_or_else(|| model_name.clone());
        let key = (
            provider_name.clone(),
            base_url.clone().unwrap_or_default(),
            raw_api_key.clone().unwrap_or_default(),
        );
        let entry = grouped.entry(key).or_insert_with(|| ProviderData {
            name: provider_name,
            base_url: base_url.clone(),
            api_key: raw_api_key
                .clone()
                .and_then(|value| maybe_mask_secret(value, mask_secrets)),
            enabled: true,
            models: Vec::new(),
            protocol: Some(WireProtocol::ChatCompletions),
            ..ProviderData::default()
        });
        if !entry.models.iter().any(|model| model.id == model_id) {
            entry.models.push(model(&model_id, &model_name));
        }
    }

    grouped.into_values().collect()
}

fn parse_provider_map(raw: &Value, mask_secrets: bool) -> Vec<ProviderData> {
    let Some(map) = raw.as_object() else {
        return Vec::new();
    };
    map.iter()
        .map(|(id, raw)| {
            let raw = raw.as_object();
            ProviderData {
                name: string(raw, "name").unwrap_or_else(|| id.clone()),
                base_url: string(raw, "url")
                    .or_else(|| string(raw, "baseURL"))
                    .or_else(|| string(raw, "baseUrl"))
                    .or_else(|| string(raw, "base_url")),
                api_key: string(raw, "apiKey")
                    .or_else(|| string(raw, "api_key"))
                    .and_then(|value| maybe_mask_secret(value, mask_secrets)),
                enabled: !raw
                    .and_then(|raw| raw.get("disabled"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                models: models(raw.and_then(|raw| raw.get("models"))),
                protocol: Some(WireProtocol::ChatCompletions),
                ..ProviderData::default()
            }
        })
        .collect()
}

fn models(raw: Option<&Value>) -> Vec<ProviderModel> {
    match raw {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(|id| model(id, id)))
            .collect(),
        Some(Value::Object(items)) => items
            .iter()
            .map(|(id, raw)| model(id, raw.get("name").and_then(Value::as_str).unwrap_or(id)))
            .collect(),
        _ => Vec::new(),
    }
}

fn model(id: &str, name: &str) -> ProviderModel {
    ProviderModel {
        id: id.to_string(),
        name: Some(name.to_string()),
        enabled: true,
    }
}

fn string(raw: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<String> {
    raw?.get(key)?.as_str().map(str::to_string)
}

fn maybe_mask_secret(value: String, mask_secrets: bool) -> Option<String> {
    if mask_secrets {
        mask_secret(Some(&value))
    } else {
        Some(value)
    }
}

fn host_from_url(value: &str) -> Option<String> {
    let rest = value.split_once("://")?.1;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())?;
    if let Some(ipv6) = authority.strip_prefix('[') {
        return ipv6.split_once(']').map(|(host, _)| host.to_string());
    }
    authority
        .split(':')
        .next()
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use crate::agents::workbuddy::provider::ProviderAsset;
    use crate::interfaces::Asset;

    #[test]
    fn masks_provider_api_key_from_models_json_array() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("models.json"),
            r#"[{"id":"MiniMax-M2.5","name":"MiniMax-M2.5","vendor":"MiniMax","url":"https://api.minimaxi.com/v1/chat/completions","apiKey":"sk-1234567890"}]"#,
        )
        .unwrap();

        let data = ProviderAsset::new("workbuddy", dir.path())
            .get_data()
            .unwrap();

        assert_eq!(data[0].name, "MiniMax");
        assert_eq!(
            data[0].base_url.as_deref(),
            Some("https://api.minimaxi.com/v1/chat/completions")
        );
        assert_ne!(data[0].api_key.as_deref(), Some("sk-1234567890"));
        assert_eq!(data[0].models[0].id, "MiniMax-M2.5");
    }

    #[test]
    fn runtime_data_keeps_provider_api_key_for_probe() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("models.json"),
            r#"[{"id":"auto","name":"TokenPlan企业轻享 / Auto","vendor":"Tencent Cloud Token Plan","url":"https://tokenhub.tencentmaas.com/plan/v3/chat/completions","apiKey":"sk-1234567890"}]"#,
        )
        .unwrap();

        let data = ProviderAsset::new("workbuddy", dir.path())
            .get_runtime_data()
            .unwrap();

        assert_eq!(data[0].api_key.as_deref(), Some("sk-1234567890"));
        assert_eq!(data[0].models[0].id, "auto");
    }
}

use std::collections::BTreeMap;

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationResult, AssetType, ProviderData, ProviderModel, ProviderProbeRequest,
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
            crate::interfaces::AssetMutationErrorCode::Unsupported,
            "CodeBuddy provider mutation is not supported",
        ))
    }
}

fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let settings = read_json_file(agent_home.join("settings.json"))?;
    let active_model = settings.as_ref().and_then(active_model_ref);
    let mut out = Vec::new();
    if let Some(config) = settings {
        out.extend(parse_providers(
            &config,
            mask_secrets,
            active_model.as_deref(),
        ));
    }
    if let Some(config) = read_json_file(agent_home.join("models.json"))? {
        out.extend(parse_providers(
            &config,
            mask_secrets,
            active_model.as_deref(),
        ));
    }
    Ok(out)
}

fn active_model_ref(raw: &Value) -> Option<String> {
    match raw.get("model")? {
        Value::String(value) => non_empty_string(value),
        Value::Object(map) => string(Some(map), "id")
            .or_else(|| string(Some(map), "model"))
            .or_else(|| string(Some(map), "modelId")),
        _ => None,
    }
}

fn parse_providers(
    raw: &Value,
    mask_secrets: bool,
    active_model: Option<&str>,
) -> Vec<ProviderData> {
    match raw {
        Value::Array(items) => parse_model_entries(items, mask_secrets, active_model),
        Value::Object(map) => {
            let mut out = Vec::new();
            if let Some(Value::Array(items)) = map.get("models") {
                out.extend(parse_model_entries(items, mask_secrets, active_model));
            }
            if let Some(providers) = map.get("providers").or_else(|| map.get("modelProviders")) {
                out.extend(parse_provider_map(providers, mask_secrets, active_model));
            }
            out
        }
        _ => Vec::new(),
    }
}

fn parse_model_entries(
    items: &[Value],
    mask_secrets: bool,
    active_model: Option<&str>,
) -> Vec<ProviderData> {
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
            enabled: active_model.is_none(),
            models: Vec::new(),
            protocol: Some(WireProtocol::ChatCompletions),
            ..ProviderData::default()
        });
        if !entry.models.iter().any(|model| model.id == model_id) {
            let enabled = model_enabled(&model_id, active_model);
            if enabled {
                entry.enabled = true;
            }
            entry.models.push(model(&model_id, &model_name, enabled));
        }
    }

    grouped.into_values().collect()
}

fn parse_provider_map(
    raw: &Value,
    mask_secrets: bool,
    active_model: Option<&str>,
) -> Vec<ProviderData> {
    let Some(map) = raw.as_object() else {
        return Vec::new();
    };
    map.iter()
        .map(|(id, raw)| {
            let raw = raw.as_object();
            let disabled = raw
                .and_then(|raw| raw.get("disabled"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let models = models(raw.and_then(|raw| raw.get("models")), active_model);
            ProviderData {
                name: string(raw, "name").unwrap_or_else(|| id.clone()),
                base_url: string(raw, "baseURL")
                    .or_else(|| string(raw, "baseUrl"))
                    .or_else(|| string(raw, "base_url")),
                api_key: string(raw, "apiKey")
                    .or_else(|| string(raw, "api_key"))
                    .and_then(|value| maybe_mask_secret(value, mask_secrets)),
                enabled: !disabled && provider_enabled(id, &models, active_model),
                models,
                protocol: None,
                ..ProviderData::default()
            }
        })
        .collect()
}

fn models(raw: Option<&Value>, active_model: Option<&str>) -> Vec<ProviderModel> {
    match raw {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .map(|id| model(id, id, model_enabled(id, active_model)))
            })
            .collect(),
        Some(Value::Object(items)) => items
            .iter()
            .map(|(id, raw)| {
                model(
                    id,
                    raw.get("name").and_then(Value::as_str).unwrap_or(id),
                    model_enabled(id, active_model),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn model(id: &str, name: &str, enabled: bool) -> ProviderModel {
    ProviderModel {
        id: id.to_string(),
        name: Some(name.to_string()),
        enabled,
    }
}

fn string(raw: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<String> {
    raw?.get(key)?.as_str().and_then(non_empty_string)
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn maybe_mask_secret(value: String, mask_secrets: bool) -> Option<String> {
    if mask_secrets {
        mask_secret(Some(&value))
    } else {
        Some(value)
    }
}

fn provider_enabled(
    provider_id: &str,
    models: &[ProviderModel],
    active_model: Option<&str>,
) -> bool {
    let Some(active_model) = active_model else {
        return true;
    };
    provider_ref_matches(active_model, provider_id) || models.iter().any(|model| model.enabled)
}

fn model_enabled(model_id: &str, active_model: Option<&str>) -> bool {
    let Some(active_model) = active_model else {
        return true;
    };
    model_ref_matches(active_model, model_id)
}

fn provider_ref_matches(active_model: &str, provider_id: &str) -> bool {
    let active_model = active_model.trim();
    let provider_id = provider_id.trim();
    if active_model.is_empty() || provider_id.is_empty() {
        return false;
    }
    active_model == provider_id
        || active_model
            .strip_prefix(provider_id)
            .is_some_and(|rest| rest.starts_with(':') || rest.starts_with('/'))
}

fn model_ref_matches(active_model: &str, model_id: &str) -> bool {
    let active_model = active_model.trim();
    let model_id = model_id.trim();
    if active_model.is_empty() || model_id.is_empty() {
        return false;
    }
    active_model == model_id
        || active_model
            .strip_suffix(model_id)
            .is_some_and(|prefix| prefix.ends_with(':') || prefix.ends_with('/'))
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
    use crate::interfaces::Asset;

    use super::ProviderAsset;

    #[test]
    fn masks_provider_api_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"providers":{"openai":{"baseURL":"https://api.example","apiKey":"sk-1234567890"}}}"#,
        )
        .unwrap();
        let data = ProviderAsset::new("codebuddy", dir.path())
            .get_data()
            .unwrap();
        assert_ne!(data[0].api_key.as_deref(), Some("sk-1234567890"));
        assert!(data[0].api_key.as_deref().unwrap().contains("****"));
    }

    #[test]
    fn runtime_data_keeps_provider_api_key_for_probe() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"providers":{"openai":{"baseURL":"https://api.example","apiKey":"sk-1234567890"}}}"#,
        )
        .unwrap();
        let data = ProviderAsset::new("codebuddy", dir.path())
            .get_runtime_data()
            .unwrap();
        assert_eq!(data[0].api_key.as_deref(), Some("sk-1234567890"));
    }

    #[test]
    fn reads_official_models_json_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("models.json"),
            r#"{"models":[{"id":"baizhi-v1","name":"Baizhi V1","vendor":"Baizhi","url":"https://ai-api-gateway.example/api/openai/chat/completions","apiKey":"sk-1234567890"}],"availableModels":["baizhi-v1"]}"#,
        )
        .unwrap();

        let data = ProviderAsset::new("codebuddy", dir.path())
            .get_data()
            .unwrap();

        assert_eq!(data.len(), 1);
        assert_eq!(data[0].name, "Baizhi");
        assert_eq!(
            data[0].base_url.as_deref(),
            Some("https://ai-api-gateway.example/api/openai/chat/completions")
        );
        assert_ne!(data[0].api_key.as_deref(), Some("sk-1234567890"));
        assert!(data[0].api_key.as_deref().unwrap().contains("****"));
        assert_eq!(data[0].models[0].id, "baizhi-v1");
    }

    #[test]
    fn settings_model_marks_active_official_provider() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"model":"custom-local:dev/gpt-5.5","trustedDirectories":["/workspace"]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("models.json"),
            r#"{"models":[{"id":"dev/gpt-5.5","name":"GPT-5.5","vendor":"Baizhi","url":"https://ai-api-gateway.example/api/openai/chat/completions","apiKey":"sk-1234567890"},{"id":"other","name":"Other","vendor":"Other","url":"https://other.example/v1/chat/completions","apiKey":"sk-other"}]}"#,
        )
        .unwrap();

        let data = ProviderAsset::new("codebuddy", dir.path())
            .get_data()
            .unwrap();

        let baizhi = data
            .iter()
            .find(|provider| provider.name == "Baizhi")
            .unwrap();
        let other = data
            .iter()
            .find(|provider| provider.name == "Other")
            .unwrap();
        assert!(baizhi.enabled);
        assert!(baizhi.models[0].enabled);
        assert!(!other.enabled);
        assert!(!other.models[0].enabled);
    }

    #[test]
    fn settings_without_provider_config_does_not_emit_provider() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"model":"custom-local:dev/gpt-5.5","trustedDirectories":["/workspace"]}"#,
        )
        .unwrap();

        let data = ProviderAsset::new("codebuddy", dir.path())
            .get_data()
            .unwrap();

        assert!(data.is_empty());
    }
}

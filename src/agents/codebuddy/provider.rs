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
    let mut out = Vec::new();
    for file in ["settings.json", "models.json"] {
        let Some(config) = read_json_file(agent_home.join(file))? else {
            continue;
        };
        out.extend(parse_providers(
            config
                .get("providers")
                .or_else(|| config.get("modelProviders")),
            mask_secrets,
        ));
    }
    Ok(out)
}

fn parse_providers(raw: Option<&Value>, mask_secrets: bool) -> Vec<ProviderData> {
    let Some(map) = raw.and_then(Value::as_object) else {
        return Vec::new();
    };
    map.iter()
        .map(|(id, raw)| {
            let raw = raw.as_object();
            ProviderData {
                name: string(raw, "name").unwrap_or_else(|| id.clone()),
                base_url: string(raw, "baseURL")
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
                protocol: None,
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
}

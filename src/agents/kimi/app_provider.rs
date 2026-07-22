use std::path::Path;

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, ProviderData, ProviderModel, ProviderProbeRequest};
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
        [
            WireProtocol::Responses,
            WireProtocol::ChatCompletions,
            WireProtocol::AnthropicMessages,
        ]
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
        provider_data(self.core.agent_home())
    }
}

fn provider_data(agent_home: &Path) -> SentraResult<Vec<ProviderData>> {
    let daimon_home = crate::agents::kimi::app_daimon_home(agent_home);
    if let Some(config) = read_json_file(daimon_home.join("config.json"))? {
        let providers = providers_from_daimon_config(&config);
        if !providers.is_empty() {
            return Ok(providers);
        }
    }

    for config_home in [
        daimon_home.join("runtime").join("kimi-code"),
        agent_home.join("daimon-share"),
    ] {
        let providers = crate::agents::kimi::provider::provider_data(&config_home, true)?;
        if !providers.is_empty() {
            return Ok(providers);
        }
    }
    Ok(Vec::new())
}

fn providers_from_daimon_config(config: &Value) -> Vec<ProviderData> {
    let Some(model_config) = config.get("model").and_then(Value::as_object) else {
        return Vec::new();
    };
    let providers = model_config.get("providers").and_then(Value::as_object);
    let models = model_config.get("models").and_then(Value::as_object);
    let current_model = model_config.get("current").and_then(Value::as_str);
    let active_provider =
        current_model.and_then(|current| models?.get(current)?.get("provider")?.as_str());
    let credentials = config.get("credentials").and_then(Value::as_object);

    let Some(providers) = providers else {
        return Vec::new();
    };
    providers
        .iter()
        .filter_map(|(provider_id, raw)| {
            let raw = raw.as_object()?;
            let credential = string_field(raw, &["credential", "credentialId"])
                .and_then(|credential| credentials?.get(&credential))
                .and_then(Value::as_object);
            let provider_models = models
                .into_iter()
                .flat_map(|models| models.iter())
                .filter_map(|(alias, model)| {
                    let model = model.as_object()?;
                    (string_field(model, &["provider"]).as_deref() == Some(provider_id)).then(
                        || ProviderModel {
                            id: string_field(model, &["model", "id"])
                                .unwrap_or_else(|| alias.clone()),
                            name: Some(alias.clone()),
                            enabled: current_model.map_or(true, |current| current == alias),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let provider_type = string_field(raw, &["type"]);
            let secret = string_field(raw, &["apiKey", "api_key", "token"]).or_else(|| {
                string_field(
                    credential?,
                    &["apiKey", "api_key", "accessToken", "access_token", "token"],
                )
            });

            Some(ProviderData {
                name: string_field(raw, &["name", "displayName"])
                    .unwrap_or_else(|| provider_id.clone()),
                provider_id: Some(provider_id.clone()),
                raw_provider_id: Some(provider_id.clone()),
                base_url: string_field(raw, &["baseUrl", "base_url", "url"])
                    .or_else(|| string_field(credential?, &["baseUrl", "base_url", "url"])),
                api_key: mask_secret(secret.as_deref()),
                enabled: active_provider.map_or(true, |active| active == provider_id),
                models: provider_models,
                protocol: provider_type.as_deref().and_then(protocol_for_type),
                ..ProviderData::default()
            })
        })
        .collect()
}

fn protocol_for_type(provider_type: &str) -> Option<WireProtocol> {
    match provider_type.to_ascii_lowercase().as_str() {
        "anthropic" | "anthropic_messages" => Some(WireProtocol::AnthropicMessages),
        "openai_responses" | "responses" => Some(WireProtocol::Responses),
        "kimi" | "openai" | "chat_completions" => Some(WireProtocol::ChatCompletions),
        _ => None,
    }
}

fn string_field(value: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_daimon_providers_models_and_masks_credentials() {
        let config = serde_json::json!({
            "model": {
                "current": "k3-agent",
                "providers": {
                    "daimon-kimi-code": {
                        "type": "kimi",
                        "baseUrl": "https://api.example.test",
                        "credential": "kimiCode"
                    },
                    "daimon-kimi-messages": {
                        "type": "anthropic",
                        "credential": "kimiCode"
                    }
                },
                "models": {
                    "k3-agent": {
                        "provider": "daimon-kimi-code",
                        "model": "k3-agent"
                    },
                    "messages": {
                        "provider": "daimon-kimi-messages",
                        "model": "k2p6-agent"
                    }
                }
            },
            "credentials": {
                "kimiCode": {
                    "apiKey": "secret-value",
                    "baseUrl": "https://credential.example.test"
                }
            }
        });

        let providers = providers_from_daimon_config(&config);
        let code = providers
            .iter()
            .find(|provider| provider.provider_id.as_deref() == Some("daimon-kimi-code"))
            .unwrap();
        let messages = providers
            .iter()
            .find(|provider| provider.provider_id.as_deref() == Some("daimon-kimi-messages"))
            .unwrap();

        assert!(code.enabled);
        assert_eq!(code.api_key.as_deref(), Some("****"));
        assert_eq!(code.protocol, Some(WireProtocol::ChatCompletions));
        assert_eq!(code.models[0].id, "k3-agent");
        assert!(!messages.enabled);
        assert_eq!(messages.protocol, Some(WireProtocol::AnthropicMessages));
        assert_eq!(
            messages.base_url.as_deref(),
            Some("https://credential.example.test")
        );
        assert!(
            !serde_json::to_string(&providers)
                .unwrap()
                .contains("secret-value")
        );
    }
}

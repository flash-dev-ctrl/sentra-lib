use serde_json::json;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest,
};
use crate::providers::{
    ProviderActivationStatus, ProviderCandidate, ProviderFieldSource, ProviderRegistry,
};
use crate::utils::protocol::WireProtocol;
use crate::utils::{backup_file, read_json_file, write_json_file};

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
        vec![ProviderProbeRequest {
            protocol: WireProtocol::AnthropicMessages,
            body: None,
            prompt: None,
        }]
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

    fn set_data(&self, value: ProviderData) -> SentraResult<AssetMutationResult> {
        let settings_path = self.core.agent_home().join("settings.json");
        let mut settings = read_json_file(&settings_path)?.unwrap_or_else(|| json!({}));
        if !settings.is_object() {
            settings = json!({});
        }
        if !settings.get("env").is_some_and(|value| value.is_object()) {
            settings["env"] = json!({});
        }
        if let Some(base_url) = value.base_url {
            settings["env"]["ANTHROPIC_BASE_URL"] = json!(base_url);
        }
        if let Some(api_key) = value.api_key {
            settings["env"]["ANTHROPIC_AUTH_TOKEN"] = json!(api_key);
            if let Some(env) = settings
                .get_mut("env")
                .and_then(|value| value.as_object_mut())
            {
                env.remove("ANTHROPIC_API_KEY");
            }
        }
        if let Some(model) = value.models.first() {
            for key in [
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
            ] {
                settings["env"][key] = json!(model.id);
            }
        }
        backup_file(&settings_path)?;
        write_json_file(settings_path, &settings)?;
        Ok(AssetMutationResult::changed())
    }

    fn del_data(&self, item: &ProviderData) -> SentraResult<AssetMutationResult> {
        let settings_path = self.core.agent_home().join("settings.json");
        let Some(mut settings) = read_json_file(&settings_path)? else {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                "settings.json does not exist",
            ));
        };
        let Some(env) = settings
            .get_mut("env")
            .and_then(|value| value.as_object_mut())
        else {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                "provider was not found in Claude Code settings",
            ));
        };
        let base_url = env
            .get("ANTHROPIC_BASE_URL")
            .and_then(|value| value.as_str());
        if base_url != item.base_url.as_deref() {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotMatched,
                "provider base URL did not match",
            ));
        }
        if let Some(api_key) = &item.api_key {
            let configured = env
                .get("ANTHROPIC_AUTH_TOKEN")
                .and_then(|value| value.as_str());
            if configured != Some(api_key) {
                return Ok(AssetMutationResult::unchanged(
                    AssetMutationErrorCode::NotMatched,
                    "provider API key did not match",
                ));
            }
        }
        for key in [
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
        ] {
            env.remove(key);
        }
        backup_file(&settings_path)?;
        write_json_file(settings_path, &settings)?;
        Ok(AssetMutationResult::changed())
    }
}

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let Some(settings) = read_json_file(agent_home.join("settings.json"))? else {
        return Ok(Vec::new());
    };
    let env = settings.get("env").and_then(|value| value.as_object());
    let Some(base_url) = env
        .and_then(|env| env.get("ANTHROPIC_BASE_URL"))
        .and_then(|value| value.as_str())
    else {
        return Ok(Vec::new());
    };
    let api_key = env
        .and_then(|env| env.get("ANTHROPIC_AUTH_TOKEN"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let mut seen = std::collections::HashSet::new();
    let mut models = Vec::new();
    for (id_key, name_key) in [
        ("ANTHROPIC_MODEL", None),
        (
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            Some("ANTHROPIC_DEFAULT_OPUS_MODEL_NAME"),
        ),
        (
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            Some("ANTHROPIC_DEFAULT_SONNET_MODEL_NAME"),
        ),
        (
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            Some("ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME"),
        ),
    ] {
        let Some(id) = env
            .and_then(|env| env.get(id_key))
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        if !seen.insert(id.to_string()) {
            continue;
        }
        let name = name_key
            .and_then(|key| env.and_then(|env| env.get(key)))
            .and_then(|value| value.as_str())
            .unwrap_or(id)
            .to_string();
        models.push(ProviderModel {
            id: id.to_string(),
            name: Some(name),
            enabled: true,
        });
    }
    let mut candidate = ProviderCandidate::new("claude-cli");
    candidate.display_name =
        Some(host_from_url(base_url).unwrap_or_else(|| "Anthropic".to_string()));
    candidate.configured_base_url = Some(base_url.to_string());
    candidate.protocol_hint = Some(WireProtocol::AnthropicMessages);
    candidate.protocol_source = Some(ProviderFieldSource::Inferred);
    candidate.api_key = api_key;
    candidate.activation = ProviderActivationStatus::Active;
    candidate.models = models;
    Ok(vec![ProviderRegistry::builtin().resolve(candidate)])
}

fn host_from_url(value: &str) -> Option<String> {
    let rest = value.split_once("://")?.1;
    rest.split(['/', '?', '#', ':'])
        .next()
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

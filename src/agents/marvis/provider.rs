use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationResult, AssetType, ProviderData, ProviderProbeRequest,
};
use crate::utils::protocol::{WireProtocol, build_model_probe_request};
use crate::utils::{mask_secret, read_text_file};

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
            "Marvis provider mutation is not supported",
        ))
    }
}

fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let mut out = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_default();
    for settings in [
        agent_home.join("settings.yaml"),
        cwd.join(".marvis").join("settings.yaml"),
    ] {
        let Some(config) = read_yaml(settings)? else {
            continue;
        };
        let root = config.as_mapping();
        let providers = yaml_field(root, "providers")
            .or_else(|| yaml_field(root, "model_providers"))
            .and_then(serde_yaml::Value::as_mapping);
        let Some(providers) = providers else {
            continue;
        };
        out.extend(providers.iter().filter_map(|(id, raw)| {
            let id = id.as_str()?;
            let raw = raw.as_mapping();
            Some(ProviderData {
                name: string(raw, "name").unwrap_or_else(|| id.to_string()),
                base_url: string(raw, "base_url").or_else(|| string(raw, "baseUrl")),
                api_key: string(raw, "api_key")
                    .or_else(|| string(raw, "apiKey"))
                    .and_then(|value| maybe_mask_secret(value, mask_secrets)),
                enabled: !bool_field(raw, "disabled").unwrap_or(false),
                models: Vec::new(),
                protocol: None,
                ..ProviderData::default()
            })
        }));
    }
    Ok(out)
}

fn read_yaml(path: impl AsRef<std::path::Path>) -> SentraResult<Option<serde_yaml::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    serde_yaml::from_str(&content).map(Some).map_err(Into::into)
}

fn string(raw: Option<&serde_yaml::Mapping>, key: &str) -> Option<String> {
    yaml_field(raw, key)?.as_str().map(str::to_string)
}

fn bool_field(raw: Option<&serde_yaml::Mapping>, key: &str) -> Option<bool> {
    yaml_field(raw, key)?.as_bool()
}

fn yaml_field<'a>(
    raw: Option<&'a serde_yaml::Mapping>,
    key: &str,
) -> Option<&'a serde_yaml::Value> {
    raw?.get(&serde_yaml::Value::String(key.to_string()))
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
    fn runtime_data_keeps_provider_api_key_for_probe() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.yaml"),
            "providers:\n  openai:\n    base_url: https://api.example\n    api_key: sk-1234567890\n",
        )
        .unwrap();
        let data = ProviderAsset::new("marvis", dir.path())
            .get_runtime_data()
            .unwrap();
        assert_eq!(data[0].api_key.as_deref(), Some("sk-1234567890"));
    }
}

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, ProviderData, ProviderModel};
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
}

impl_erased_asset!(ProviderAsset, AssetType::Provider, Vec<ProviderData>);

impl Asset<Vec<ProviderData>> for ProviderAsset {
    fn get_data(&self) -> SentraResult<Vec<ProviderData>> {
        read_provider_data(true)
    }

    fn get_runtime_data(&self) -> SentraResult<Vec<ProviderData>> {
        read_provider_data(false)
    }
}

fn read_provider_data(mask_secrets: bool) -> SentraResult<Vec<ProviderData>> {
    let Some(path) = crate::agents::trae::workspace_path("trae_config.yaml") else {
        return Ok(Vec::new());
    };
    let Some(content) = read_text_file(path)? else {
        return Ok(Vec::new());
    };
    let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
        return Ok(Vec::new());
    };
    Ok(provider_data(&config, mask_secrets))
}

fn provider_data(config: &serde_yaml::Value, mask_secrets: bool) -> Vec<ProviderData> {
    let Some(providers) = config
        .get("model_providers")
        .and_then(|value| value.as_mapping())
    else {
        return Vec::new();
    };
    providers
        .iter()
        .filter_map(|(key, raw)| {
            let id = key.as_str()?.to_string();
            Some(ProviderData {
                name: id.clone(),
                provider_id: Some(id.clone()),
                raw_provider_id: Some(id),
                base_url: yaml_string(raw, "base_url").or_else(|| yaml_string(raw, "baseUrl")),
                api_key: yaml_string(raw, "api_key")
                    .or_else(|| yaml_string(raw, "apiKey"))
                    .and_then(|value| maybe_mask_secret(value, mask_secrets)),
                enabled: true,
                models: yaml_string(raw, "model")
                    .into_iter()
                    .map(|id| ProviderModel {
                        id,
                        name: None,
                        enabled: true,
                    })
                    .collect(),
                ..ProviderData::default()
            })
        })
        .collect()
}

fn maybe_mask_secret(value: String, mask_secrets: bool) -> Option<String> {
    if mask_secrets {
        mask_secret(Some(&value))
    } else {
        Some(value)
    }
}

fn yaml_string(value: &serde_yaml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::provider_data;

    #[test]
    fn runtime_provider_data_keeps_api_key() {
        let config: serde_yaml::Value =
            serde_yaml::from_str("model_providers:\n  openai:\n    api_key: sk-trae-secret\n")
                .unwrap();
        let display = provider_data(&config, true);
        let runtime = provider_data(&config, false);

        assert_ne!(display[0].api_key, runtime[0].api_key);
        assert_eq!(runtime[0].api_key.as_deref(), Some("sk-trae-secret"));
    }
}

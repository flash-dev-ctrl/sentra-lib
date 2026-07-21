use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, ProviderData, ProviderModel};
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
}

impl_erased_asset!(ProviderAsset, AssetType::Provider, Vec<ProviderData>);

impl Asset<Vec<ProviderData>> for ProviderAsset {
    fn get_data(&self) -> SentraResult<Vec<ProviderData>> {
        provider_data(self.core.agent_home(), true)
    }

    fn get_runtime_data(&self) -> SentraResult<Vec<ProviderData>> {
        provider_data(self.core.agent_home(), false)
    }
}

fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let mut results = Vec::new();
    for path in [
        agent_home.join("config.json"),
        agent_home.join("config"),
        agent_home.join("config").join("config.json"),
        agent_home.join("settings.json"),
    ] {
        if !path.is_file() {
            continue;
        }
        let Some(config) = read_json_file(&path)? else {
            continue;
        };
        results.extend(providers_from_value(&config, mask_secrets));
    }
    Ok(results)
}

fn providers_from_value(config: &serde_json::Value, mask_secrets: bool) -> Vec<ProviderData> {
    let Some(raw) = config
        .get("providers")
        .or_else(|| config.get("modelProviders"))
        .or_else(|| {
            config
                .get("models")
                .and_then(|models| models.get("providers"))
        })
    else {
        return Vec::new();
    };
    let entries: Vec<(String, &serde_json::Value)> = match raw {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(key, value)| (key.clone(), value))
            .collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, value)| (i.to_string(), value))
            .collect(),
        _ => Vec::new(),
    };
    entries
        .into_iter()
        .map(|(name, raw)| {
            let obj = raw.as_object();
            ProviderData {
                name: string_field(obj, &["name"]).unwrap_or(name),
                base_url: string_field(obj, &["baseUrl", "base_url", "url"]),
                api_key: string_field(
                    obj,
                    &["apiKey", "api_key", "key", "token", "password", "secret"],
                )
                .and_then(|value| maybe_mask_secret(value, mask_secrets)),
                enabled: obj
                    .and_then(|raw| raw.get("enabled"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                models: models(obj.and_then(|raw| raw.get("models"))),
                protocol: None,
                ..ProviderData::default()
            }
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

fn models(raw: Option<&serde_json::Value>) -> Vec<ProviderModel> {
    raw.and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let id = value
                .as_str()
                .or_else(|| value.get("id").and_then(|value| value.as_str()))
                .or_else(|| value.get("model").and_then(|value| value.as_str()))?;
            Some(ProviderModel {
                id: id.to_string(),
                name: value
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                enabled: true,
            })
        })
        .collect()
}

fn string_field(
    raw: Option<&serde_json::Map<String, serde_json::Value>>,
    keys: &[&str],
) -> Option<String> {
    let raw = raw?;
    keys.iter()
        .find_map(|key| raw.get(*key).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::providers_from_value;

    #[test]
    fn runtime_provider_data_keeps_api_key() {
        let config = serde_json::json!({
            "providers": {"openai": {"apiKey": "sk-lingcode-secret"}}
        });
        let display = providers_from_value(&config, true);
        let runtime = providers_from_value(&config, false);

        assert_ne!(display[0].api_key, runtime[0].api_key);
        assert_eq!(runtime[0].api_key.as_deref(), Some("sk-lingcode-secret"));
    }
}

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
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
        let mut results = Vec::new();
        for path in [
            self.core.agent_home().join("config.json"),
            super::user_home(self.core.agent_home())
                .join(".gemini")
                .join("config")
                .join("config.json"),
        ] {
            let Some(config) = read_json_file(path)? else {
                continue;
            };
            collect_providers(&config, &mut results);
        }
        Ok(results)
    }

    fn set_data(&self, _value: ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Antigravity provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Antigravity provider mutation is not supported",
        ))
    }
}

fn collect_providers(value: &serde_json::Value, results: &mut Vec<ProviderData>) {
    let Some(map) = value
        .get("providers")
        .or_else(|| value.get("modelProviders"))
        .and_then(|value| value.as_object())
    else {
        return;
    };
    for (name, raw) in map {
        let raw = raw.as_object();
        results.push(ProviderData {
            name: raw
                .and_then(|raw| raw.get("name").and_then(|value| value.as_str()))
                .unwrap_or(name)
                .to_string(),
            base_url: string_field(raw, &["baseUrl", "base_url", "url"]),
            api_key: string_field(raw, &["apiKey", "api_key", "key", "token"])
                .and_then(|value| mask_secret(Some(&value))),
            enabled: raw
                .and_then(|raw| raw.get("enabled"))
                .and_then(|value| value.as_bool())
                .unwrap_or(true),
            models: models(raw.and_then(|raw| raw.get("models"))),
            protocol: None,
            ..ProviderData::default()
        });
    }
}

fn string_field(
    raw: Option<&serde_json::Map<String, serde_json::Value>>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| raw?.get(*key).and_then(|value| value.as_str()))
        .map(str::to_string)
}

fn models(raw: Option<&serde_json::Value>) -> Vec<ProviderModel> {
    raw.and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str())
        .map(|id| ProviderModel {
            id: id.to_string(),
            name: Some(id.to_string()),
            enabled: true,
        })
        .collect()
}

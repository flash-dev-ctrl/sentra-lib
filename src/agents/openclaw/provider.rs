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
        provider_data(self.core.agent_home())
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

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let Some(config) = read_json_file(agent_home.join("openclaw.json"))? else {
        return Ok(Vec::new());
    };
    let providers = config
        .get("models")
        .and_then(|models| models.get("providers"))
        .or_else(|| config.get("providers"))
        .or_else(|| config.get("modelProviders"));
    let Some(providers) = providers else {
        return Ok(Vec::new());
    };
    let entries: Vec<(String, &serde_json::Value)> = if let Some(items) = providers.as_array() {
        items
            .iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), value))
            .collect()
    } else if let Some(map) = providers.as_object() {
        map.iter()
            .map(|(key, value)| (key.clone(), value))
            .collect()
    } else {
        Vec::new()
    };

    Ok(entries
        .into_iter()
        .map(|(name, value)| {
            let raw = value.as_object();
            ProviderData {
                name,
                base_url: raw
                    .and_then(|raw| {
                        raw.get("baseUrl")
                            .or_else(|| raw.get("base_url"))
                            .and_then(|value| value.as_str())
                    })
                    .map(str::to_string),
                api_key: mask_secret(raw.and_then(|raw| {
                    raw.get("apiKey")
                        .or_else(|| raw.get("api_key"))
                        .and_then(|value| value.as_str())
                })),
                enabled: raw
                    .and_then(|raw| raw.get("enabled"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                models: provider_models(raw.and_then(|raw| raw.get("models"))),
                protocol: None,
                ..ProviderData::default()
            }
        })
        .collect())
}

fn provider_models(raw: Option<&serde_json::Value>) -> Vec<ProviderModel> {
    let Some(items) = raw.and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|model| {
            let item = model.as_object()?;
            let id = item.get("id").and_then(|value| value.as_str())?;
            Some(ProviderModel {
                id: id.to_string(),
                name: item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .or_else(|| Some(id.to_string())),
                enabled: true,
            })
        })
        .collect()
}

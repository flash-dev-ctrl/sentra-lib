use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
};
use crate::utils::{mask_secret, read_text_file};

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
            "Hermes provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Hermes provider mutation is not supported",
        ))
    }
}

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let Some(config) = read_yaml_file(&agent_home.join("config.yaml"))? else {
        return Ok(Vec::new());
    };
    let json = serde_json::to_value(config).unwrap_or_default();
    let mut results = Vec::new();
    let mut fallback_models = std::collections::BTreeMap::<String, Vec<String>>::new();

    let Some(map) = json.as_object() else {
        return Ok(results);
    };
    for (key, value) in map {
        let Some(prefix) = key.strip_suffix("_providers") else {
            continue;
        };
        let Some(items) = value.as_array() else {
            continue;
        };
        for (index, item) in items.iter().enumerate() {
            let Some(raw) = item.as_object() else {
                continue;
            };
            let provider_ref = raw
                .get("provider")
                .and_then(|value| value.as_str())
                .map(|value| value.strip_prefix("custom:").unwrap_or(value).to_string());
            let model_ref = raw
                .get("model")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            if !raw.contains_key("base_url")
                && !raw.contains_key("api_key")
                && !raw.contains_key("name")
                && let (Some(provider_ref), Some(model_ref)) = (&provider_ref, &model_ref)
            {
                fallback_models
                    .entry(provider_ref.clone())
                    .or_default()
                    .push(model_ref.clone());
                continue;
            }
            add_provider(
                &mut results,
                ProviderData {
                    name: raw
                        .get("name")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{prefix}-{index}")),
                    base_url: raw
                        .get("base_url")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    api_key: mask_secret(raw.get("api_key").and_then(|value| value.as_str())),
                    enabled: true,
                    models: model_list(raw.get("models")),
                    protocol: None,
                    ..ProviderData::default()
                },
            );
        }
    }

    for (name, models) in fallback_models {
        add_provider(
            &mut results,
            ProviderData {
                name,
                base_url: None,
                api_key: None,
                enabled: true,
                models: models
                    .into_iter()
                    .map(|id| ProviderModel {
                        name: Some(id.clone()),
                        id,
                        enabled: true,
                    })
                    .collect(),
                protocol: None,
                ..ProviderData::default()
            },
        );
    }

    if let Some(raw) = json.get("model").and_then(|value| value.as_object()) {
        let name = raw
            .get("provider")
            .and_then(|value| value.as_str())
            .map(|value| value.strip_prefix("custom:").unwrap_or(value).to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let id = raw.get("default").and_then(|value| value.as_str());
        add_provider(
            &mut results,
            ProviderData {
                name,
                base_url: raw
                    .get("base_url")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                api_key: None,
                enabled: true,
                models: id
                    .map(|id| {
                        vec![ProviderModel {
                            id: id.to_string(),
                            name: Some(id.to_string()),
                            enabled: true,
                        }]
                    })
                    .unwrap_or_default(),
                protocol: None,
                ..ProviderData::default()
            },
        );
    }

    Ok(results)
}

fn model_list(raw: Option<&serde_json::Value>) -> Vec<ProviderModel> {
    let Some(map) = raw.and_then(|value| value.as_object()) else {
        return Vec::new();
    };
    map.iter()
        .map(|(id, value)| ProviderModel {
            id: id.clone(),
            name: value
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .or_else(|| Some(id.clone())),
            enabled: true,
        })
        .collect()
}

fn add_provider(results: &mut Vec<ProviderData>, provider: ProviderData) {
    let Some(existing) = results.iter_mut().find(|item| item.name == provider.name) else {
        results.push(provider);
        return;
    };
    let mut seen = existing
        .models
        .iter()
        .map(|model| model.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for model in provider.models {
        if seen.insert(model.id.clone()) {
            existing.models.push(model);
        }
    }
    if existing.base_url.is_none() {
        existing.base_url = provider.base_url;
    }
    if existing.api_key.is_none() {
        existing.api_key = provider.api_key;
    }
}

fn read_yaml_file(path: &std::path::Path) -> SentraResult<Option<serde_yaml::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    serde_yaml::from_str(&content).map(Some).map_err(Into::into)
}

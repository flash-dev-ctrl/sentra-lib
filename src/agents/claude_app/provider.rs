use serde_json::json;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest,
};
use crate::utils::protocol::WireProtocol;
use crate::utils::{backup_file, mask_secret, read_json_file, write_json_file};

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
        provider_data(self.core.agent_home(), true)
    }

    fn get_runtime_data(&self) -> SentraResult<Vec<ProviderData>> {
        provider_data(self.core.agent_home(), false)
    }

    fn set_data(&self, value: ProviderData) -> SentraResult<AssetMutationResult> {
        let config_path = writable_config_path(self.core.agent_home())?;
        let mut config = read_json_file(&config_path)?.unwrap_or_else(|| json!({}));
        if !config.is_object() {
            config = json!({});
        }
        if let Some(base_url) = value.base_url
            && base_url != "https://api.anthropic.com"
        {
            config["inferenceGatewayBaseUrl"] = json!(base_url);
        }
        if !value.models.is_empty() {
            config["inferenceModels"] = json!(
                value
                    .models
                    .into_iter()
                    .map(|model| {
                        if model.name.as_deref().is_some_and(|name| name != model.id) {
                            json!({"name": model.id, "labelOverride": model.name})
                        } else {
                            json!({"name": model.id})
                        }
                    })
                    .collect::<Vec<_>>()
            );
        }
        backup_file(&config_path)?;
        write_json_file(config_path, &config)?;
        Ok(AssetMutationResult::changed())
    }

    fn del_data(&self, item: &ProviderData) -> SentraResult<AssetMutationResult> {
        let Some(config_path) = find_config_file(self.core.agent_home())? else {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                "configLibrary provider config was not found",
            ));
        };
        let Some(mut config) = read_json_file(&config_path)? else {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotFound,
                "configLibrary provider config was not found",
            ));
        };
        let base_url = config
            .get("inferenceGatewayBaseUrl")
            .and_then(|value| value.as_str());
        if base_url != item.base_url.as_deref() {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotMatched,
                "provider base URL did not match",
            ));
        }
        if let Some(api_key) = &item.api_key
            && config
                .get("inferenceGatewayApiKey")
                .and_then(|value| value.as_str())
                != Some(api_key)
        {
            return Ok(AssetMutationResult::unchanged(
                AssetMutationErrorCode::NotMatched,
                "provider API key did not match",
            ));
        }
        if let Some(obj) = config.as_object_mut() {
            obj.remove("inferenceGatewayBaseUrl");
            obj.remove("inferenceGatewayApiKey");
            obj.remove("inferenceModels");
        }
        backup_file(&config_path)?;
        write_json_file(config_path, &config)?;
        Ok(AssetMutationResult::changed())
    }
}

fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let Some(config_path) = find_config_file(agent_home)? else {
        return Ok(Vec::new());
    };
    let Some(config) = read_json_file(config_path)? else {
        return Ok(Vec::new());
    };
    let Some(base_url) = config
        .get("inferenceGatewayBaseUrl")
        .and_then(|value| value.as_str())
    else {
        return Ok(Vec::new());
    };
    let models = config
        .get("inferenceModels")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = item.get("name").and_then(|value| value.as_str())?;
                    let name = item
                        .get("labelOverride")
                        .and_then(|value| value.as_str())
                        .unwrap_or(id);
                    Some(ProviderModel {
                        id: id.to_string(),
                        name: Some(name.to_string()),
                        enabled: true,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(vec![ProviderData {
        name: "Anthropic".to_string(),
        base_url: Some(base_url.to_string()),
        api_key: config
            .get("inferenceGatewayApiKey")
            .and_then(|value| value.as_str())
            .and_then(|value| {
                if mask_secrets {
                    mask_secret(Some(value))
                } else {
                    Some(value.to_string())
                }
            }),
        enabled: true,
        models,
        protocol: None,
        ..ProviderData::default()
    }])
}

fn find_config_file(agent_home: &std::path::Path) -> SentraResult<Option<std::path::PathBuf>> {
    let config_dir = agent_home.join("configLibrary");
    let mut candidates = Vec::new();
    if let Some(applied_id) = read_json_file(config_dir.join("_meta.json"))?.and_then(|meta| {
        meta.get("appliedId")
            .and_then(|value| value.as_str())
            .map(str::to_string)
    }) {
        candidates.push(format!("{applied_id}.json"));
    }
    for entry in std::fs::read_dir(&config_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".json") && name != "_meta.json" {
            candidates.push(name);
        }
    }
    for file in candidates {
        let path = config_dir.join(file);
        let Some(config) = read_json_file(&path)? else {
            continue;
        };
        if config
            .get("inferenceModels")
            .and_then(|value| value.as_array())
            .is_some()
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn writable_config_path(agent_home: &std::path::Path) -> SentraResult<std::path::PathBuf> {
    if let Some(applied_id) = read_json_file(agent_home.join("configLibrary").join("_meta.json"))?
        .and_then(|meta| {
            meta.get("appliedId")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
    {
        return Ok(agent_home
            .join("configLibrary")
            .join(format!("{applied_id}.json")));
    }
    Ok(agent_home.join("configLibrary").join("default.json"))
}

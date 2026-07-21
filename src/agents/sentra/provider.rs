use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::config::SENTRA_CONFIG_FILE_NAME;
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest,
};
use crate::utils::protocol::{WireProtocol, default_model_probe_prompt, parse_wire_protocol};
use crate::utils::{backup_file, mask_secret, read_text_file, write_json_file};

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
        [
            WireProtocol::Responses,
            WireProtocol::ChatCompletions,
            WireProtocol::AnthropicMessages,
        ]
        .into_iter()
        .map(|protocol| ProviderProbeRequest {
            protocol,
            body: None,
            prompt: Some(default_model_probe_prompt()),
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

    fn set_data(&self, value: ProviderData) -> SentraResult<AssetMutationResult> {
        set_provider_data(self.core.agent_home(), value)
    }

    fn del_data(&self, item: &ProviderData) -> SentraResult<AssetMutationResult> {
        delete_provider_data(self.core.agent_home(), item)
    }
}

fn provider_data(home: &std::path::Path, mask_secrets: bool) -> SentraResult<Vec<ProviderData>> {
    let config = load_config(home)?;
    let Some(llm) = config.get("llm").and_then(|value| value.as_object()) else {
        return Ok(Vec::new());
    };
    let (Some(api), Some(key), Some(model)) = (
        llm.get("api").and_then(|value| value.as_str()),
        llm.get("key").and_then(|value| value.as_str()),
        llm.get("model").and_then(|value| value.as_str()),
    ) else {
        return Ok(Vec::new());
    };
    Ok(vec![ProviderData {
        name: provider_name(api),
        base_url: Some(api.to_string()),
        api_key: if mask_secrets {
            mask_secret(Some(key))
        } else {
            Some(key.to_string())
        },
        enabled: true,
        protocol: llm
            .get("protocol")
            .and_then(|value| value.as_str())
            .and_then(|value| parse_wire_protocol(value).ok()),
        models: vec![ProviderModel {
            id: model.to_string(),
            name: Some(model.to_string()),
            enabled: true,
        }],
        ..ProviderData::default()
    }])
}

fn set_provider_data(
    home: &std::path::Path,
    provider: ProviderData,
) -> SentraResult<AssetMutationResult> {
    let Some(model) = provider
        .models
        .iter()
        .find(|item| item.enabled)
        .or_else(|| provider.models.first())
        .filter(|model| !model.id.is_empty())
    else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider model is required",
        ));
    };
    let Some(base_url) = provider.base_url.as_deref() else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider baseUrl is required",
        ));
    };
    let Some(api_key) = provider.api_key.as_deref() else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider apiKey is required",
        ));
    };
    let path = home.join(SENTRA_CONFIG_FILE_NAME);
    let mut config = load_config(home)?;
    if !config.is_object() {
        config = serde_json::json!({});
    }
    config["llm"] = serde_json::json!({
        "api": base_url,
        "key": api_key,
        "model": model.id,
    });
    if let Some(protocol) = provider.protocol {
        config["llm"]["protocol"] = serde_json::to_value(protocol)?;
    }
    backup_file(&path)?;
    write_json_file(path, &config)?;
    Ok(AssetMutationResult::changed())
}

fn delete_provider_data(
    home: &std::path::Path,
    provider: &ProviderData,
) -> SentraResult<AssetMutationResult> {
    let path = home.join(SENTRA_CONFIG_FILE_NAME);
    let Some(mut config) = read_config_file(&path)? else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::NotFound,
            format!("config file does not exist: {}", path.display()),
        ));
    };
    let Some(llm) = config.get("llm").and_then(|value| value.as_object()) else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::NotFound,
            "llm config does not exist",
        ));
    };
    let Some(api) = llm.get("api").and_then(|value| value.as_str()) else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "llm.api is required to match provider",
        ));
    };
    if provider
        .base_url
        .as_deref()
        .is_some_and(|base_url| base_url != api)
    {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::NotMatched,
            "provider baseUrl does not match current sentra config",
        ));
    }
    if let Some(object) = config.as_object_mut() {
        object.remove("llm");
    }
    backup_file(&path)?;
    write_json_file(path, &config)?;
    Ok(AssetMutationResult::changed())
}

fn load_config(home: &std::path::Path) -> SentraResult<serde_json::Value> {
    Ok(read_config_file(&home.join(SENTRA_CONFIG_FILE_NAME))?
        .unwrap_or_else(|| serde_json::json!({})))
}

fn read_config_file(path: &std::path::Path) -> SentraResult<Option<serde_json::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    Ok(serde_json::from_str::<serde_json::Value>(&content)
        .ok()
        .filter(|value| value.is_object())
        .or_else(|| Some(serde_json::json!({}))))
}

fn provider_name(base_url: &str) -> String {
    host_from_url(base_url).unwrap_or_else(|| base_url.to_string())
}

fn host_from_url(value: &str) -> Option<String> {
    let rest = value.split_once("://")?.1;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())?;
    if let Some(ipv6) = authority.strip_prefix('[') {
        return ipv6.split_once(']').map(|(host, _)| host.to_string());
    }
    authority
        .split(':')
        .next()
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use crate::agents::sentra::provider::ProviderAsset;
    use crate::config::SENTRA_CONFIG_FILE_NAME;
    use crate::interfaces::Asset;

    #[test]
    fn display_masks_api_key_while_runtime_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SENTRA_CONFIG_FILE_NAME),
            r#"{"llm":{"api":"https://api.example.test/v1","key":"sk-sentra-secret","model":"gpt-test"}}"#,
        )
        .unwrap();
        let asset = ProviderAsset::new("sentra", dir.path());

        let display = asset.get_data().unwrap();
        let runtime = asset.get_runtime_data().unwrap();

        assert_ne!(display[0].api_key.as_deref(), Some("sk-sentra-secret"));
        assert_eq!(runtime[0].api_key.as_deref(), Some("sk-sentra-secret"));
    }
}

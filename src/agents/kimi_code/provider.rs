use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest, ProviderType,
};
use crate::utils::protocol::{WireProtocol, build_model_probe_request};
use crate::utils::{backup_file, mask_secret, read_text_file, write_text_file};
use crate::{SentraError, SentraResult};

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

fn set_provider_data(
    agent_home: &std::path::Path,
    provider: ProviderData,
) -> SentraResult<AssetMutationResult> {
    if provider.provider_type != ProviderType::Gateway {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Kimi Code account provider mutation is not supported",
        ));
    }
    let Some(model) = provider
        .models
        .iter()
        .find(|item| item.enabled)
        .or_else(|| provider.models.first())
        .filter(|model| !model.id.trim().is_empty())
    else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider model is required",
        ));
    };
    let model_id = model.id.trim().to_string();
    let Some(base_url) = provider
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider baseUrl is required",
        ));
    };
    let Some(api_key) = provider
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider apiKey is required",
        ));
    };

    let provider_id = provider_config_id(&provider);
    let alias = model_alias(&model_id);
    let mut config = load_config(agent_home)?;
    config.insert(
        "default_model".to_string(),
        toml::Value::String(alias.clone()),
    );

    {
        let providers = table_field(&mut config, "providers");
        let mut entry = providers
            .remove(&provider_id)
            .and_then(table_value)
            .unwrap_or_default();
        entry.insert(
            "type".to_string(),
            toml::Value::String(provider_type_for_protocol(provider.protocol).to_string()),
        );
        entry.insert(
            "base_url".to_string(),
            toml::Value::String(base_url.to_string()),
        );
        entry.insert(
            "api_key".to_string(),
            toml::Value::String(api_key.to_string()),
        );
        providers.insert(provider_id.clone(), toml::Value::Table(entry));
    }

    {
        let models = table_field(&mut config, "models");
        let mut entry = models
            .remove(&alias)
            .and_then(table_value)
            .unwrap_or_default();
        entry.insert(
            "provider".to_string(),
            toml::Value::String(provider_id.clone()),
        );
        entry.insert("model".to_string(), toml::Value::String(model_id));
        models.insert(alias, toml::Value::Table(entry));
    }

    save_config(agent_home, &config)?;
    Ok(AssetMutationResult::changed())
}

fn delete_provider_data(
    agent_home: &std::path::Path,
    provider: &ProviderData,
) -> SentraResult<AssetMutationResult> {
    let config_path = agent_home.join("config.toml");
    let Some(content) = read_text_file(&config_path)? else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::NotFound,
            format!("config file does not exist: {}", config_path.display()),
        ));
    };
    let mut config = toml::from_str::<toml::Table>(&content).unwrap_or_default();
    let requested_id = provider_config_id_opt(provider);
    let mut removed_ids = Vec::new();
    if let Some(toml::Value::Table(providers)) = config.get_mut("providers") {
        let ids = providers
            .iter()
            .filter_map(|(id, value)| {
                let toml::Value::Table(entry) = value else {
                    return None;
                };
                provider_matches(id, entry, provider, requested_id.as_deref()).then(|| id.clone())
            })
            .collect::<Vec<_>>();
        for id in &ids {
            providers.remove(id);
        }
        removed_ids = ids;
    }

    if removed_ids.is_empty() {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::NotFound,
            "provider was not found in kimi code config",
        ));
    }

    let mut removed_aliases = Vec::new();
    if let Some(toml::Value::Table(models)) = config.get_mut("models") {
        let aliases = models
            .iter()
            .filter_map(|(alias, value)| {
                let entry = value.as_table()?;
                let provider = string_field(Some(entry), &["provider"])?;
                removed_ids
                    .iter()
                    .any(|removed| removed == &provider)
                    .then(|| alias.clone())
            })
            .collect::<Vec<_>>();
        for alias in &aliases {
            models.remove(alias);
        }
        removed_aliases = aliases;
    }

    let default_model = config
        .get("default_model")
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    if default_model
        .as_ref()
        .is_some_and(|default| removed_aliases.iter().any(|alias| alias == default))
    {
        config.remove("default_model");
    }
    if empty_table(&config, "providers") {
        config.remove("providers");
    }
    if empty_table(&config, "models") {
        config.remove("models");
    }
    save_config(agent_home, &config)?;
    Ok(AssetMutationResult::changed())
}

pub(super) fn provider_data(
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let Some(content) = read_text_file(agent_home.join("config.toml"))? else {
        return Ok(Vec::new());
    };
    let Ok(config) = toml::from_str::<toml::Value>(&content) else {
        return Ok(Vec::new());
    };
    let providers = config
        .get("providers")
        .and_then(toml::Value::as_table)
        .cloned()
        .unwrap_or_default();
    let models = model_map(config.get("models"));
    let default_model = string_field(config.as_table(), &["default_model", "model"]);
    let active_provider = default_model
        .as_deref()
        .and_then(|model| models.iter().find(|item| item.alias == model))
        .map(|model| model.provider.clone());

    let mut results = Vec::new();
    for (provider_id, raw) in providers {
        let raw = raw.as_table();
        let mut provider_models = models
            .iter()
            .filter(|model| model.provider == provider_id)
            .map(|model| ProviderModel {
                id: model.id.clone(),
                name: Some(model.alias.clone()),
                enabled: true,
            })
            .collect::<Vec<_>>();
        if provider_models.is_empty()
            && let Some(default_model) = default_model.as_deref()
            && active_provider.as_deref() == Some(&provider_id)
        {
            provider_models.push(ProviderModel {
                id: default_model.to_string(),
                name: Some(default_model.to_string()),
                enabled: true,
            });
        }
        results.push(ProviderData {
            name: string_field(raw, &["name", "display_name", "displayName"])
                .unwrap_or_else(|| provider_id.clone()),
            provider_id: Some(provider_id.clone()),
            raw_provider_id: Some(provider_id.clone()),
            base_url: string_field(raw, &["base_url", "baseURL", "baseUrl", "url"]),
            api_key: string_field(raw, &["api_key", "apiKey", "key", "token"])
                .and_then(|value| maybe_mask_secret(value, mask_secrets)),
            enabled: provider_enabled(active_provider.as_deref(), &provider_id),
            models: provider_models,
            protocol: None,
            ..ProviderData::default()
        });
    }
    Ok(results)
}

#[derive(Debug, Clone)]
struct ConfigModel {
    alias: String,
    provider: String,
    id: String,
}

fn model_map(raw: Option<&toml::Value>) -> Vec<ConfigModel> {
    let Some(table) = raw.and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    table
        .iter()
        .filter_map(|(alias, raw)| {
            let raw = raw.as_table()?;
            let provider = string_field(Some(raw), &["provider"])?;
            let id = string_field(Some(raw), &["model", "id"]).unwrap_or_else(|| alias.clone());
            Some(ConfigModel {
                alias: alias.clone(),
                provider,
                id,
            })
        })
        .collect()
}

fn provider_enabled(active_provider: Option<&str>, provider_id: &str) -> bool {
    match active_provider {
        Some(active) => active == provider_id,
        None => true,
    }
}

fn load_config(agent_home: &std::path::Path) -> SentraResult<toml::Table> {
    let Some(content) = read_text_file(agent_home.join("config.toml"))? else {
        return Ok(toml::Table::new());
    };
    Ok(toml::from_str::<toml::Table>(&content).unwrap_or_default())
}

fn save_config(agent_home: &std::path::Path, config: &toml::Table) -> SentraResult<()> {
    let config_path = agent_home.join("config.toml");
    backup_file(&config_path)?;
    let content =
        toml::to_string_pretty(config).map_err(|err| SentraError::Message(err.to_string()))?;
    write_text_file(config_path, &content)
}

fn table_field<'a>(config: &'a mut toml::Table, key: &str) -> &'a mut toml::Table {
    if !config.get(key).is_some_and(toml::Value::is_table) {
        config.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    config
        .get_mut(key)
        .and_then(toml::Value::as_table_mut)
        .expect("field is table")
}

fn table_value(value: toml::Value) -> Option<toml::Table> {
    match value {
        toml::Value::Table(table) => Some(table),
        _ => None,
    }
}

fn provider_config_id(provider: &ProviderData) -> String {
    provider_config_id_opt(provider).unwrap_or_else(|| "managed:kimi-code".to_string())
}

fn provider_config_id_opt(provider: &ProviderData) -> Option<String> {
    provider
        .raw_provider_id
        .as_deref()
        .or(provider.provider_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn model_alias(model_id: &str) -> String {
    if model_id.starts_with("kimi-code/") {
        model_id.to_string()
    } else {
        format!("kimi-code/{model_id}")
    }
}

fn provider_type_for_protocol(protocol: Option<WireProtocol>) -> &'static str {
    match protocol {
        Some(WireProtocol::Responses) => "openai_responses",
        Some(WireProtocol::AnthropicMessages) => "anthropic",
        Some(WireProtocol::ChatCompletions) | None => "kimi",
    }
}

fn provider_matches(
    id: &str,
    entry: &toml::Table,
    provider: &ProviderData,
    requested_id: Option<&str>,
) -> bool {
    if requested_id == Some(id) {
        return true;
    }
    let Some(base_url) = provider
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    string_field(Some(entry), &["base_url", "baseURL", "baseUrl", "url"]).as_deref()
        == Some(base_url)
}

fn empty_table(config: &toml::Table, key: &str) -> bool {
    config
        .get(key)
        .and_then(toml::Value::as_table)
        .is_some_and(toml::Table::is_empty)
}

fn string_field(raw: Option<&toml::Table>, keys: &[&str]) -> Option<String> {
    let raw = raw?;
    keys.iter()
        .find_map(|key| raw.get(*key).and_then(toml::Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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
    use crate::agents::kimi_code::provider::ProviderAsset;
    use crate::utils::protocol::WireProtocol;

    #[test]
    fn probe_requests_cover_supported_protocols() {
        let provider = ProviderAsset::new("kimi-cli", ".kimi-code");
        let requests = provider.get_request("kimi-k2");

        assert_eq!(requests.len(), 3);
        assert!(
            requests
                .iter()
                .any(|request| request.protocol == WireProtocol::Responses)
        );
        assert!(
            requests
                .iter()
                .any(|request| request.protocol == WireProtocol::ChatCompletions)
        );
        assert!(
            requests
                .iter()
                .any(|request| request.protocol == WireProtocol::AnthropicMessages)
        );
        assert!(requests.iter().all(|request| request.body.is_some()));
    }
}

use serde_json::json;

use crate::SentraError;
use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest,
};
use crate::utils::protocol::{WireProtocol, default_model_probe_prompt};
use crate::utils::{backup_file, read_json_file, read_text_file, write_json_file, write_text_file};

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
        let prompt = default_model_probe_prompt();
        let body = json!({
            "model": model,
            "stream": true,
            "max_output_tokens": 1024,
            "instructions": prompt.system,
            "input": [
                {
                    "role": "developer",
                    "type": "message",
                    "content": [
                        {
                            "type": "input_text",
                            "text": prompt.system
                        }
                    ]
                },
                {
                    "role": "user",
                    "type": "message",
                    "content": [
                        {
                            "type": "input_text",
                            "text": prompt.user
                        }
                    ]
                }
            ]
        })
        .to_string();
        vec![ProviderProbeRequest {
            protocol: WireProtocol::Responses,
            body: Some(body),
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
        let Some(content) = read_text_file(self.core.agent_home().join("config.toml"))? else {
            return Ok(Vec::new());
        };
        let Ok(cfg) = toml::from_str::<toml::Value>(&content) else {
            return Ok(Vec::new());
        };
        let current_model = cfg.get("model").and_then(|value| value.as_str());
        let active_id = cfg.get("model_provider").and_then(|value| value.as_str());
        let mut providers = Vec::new();
        if let Some(table) = cfg
            .get("model_providers")
            .and_then(|value| value.as_table())
        {
            for (id, raw) in table {
                let Some(base_url) = raw.get("base_url").and_then(|value| value.as_str()) else {
                    continue;
                };
                let enabled = active_id == Some(id.as_str());
                let model_name = current_model.filter(|_| enabled);
                providers.push(ProviderData {
                    name: raw
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or(id)
                        .to_string(),
                    base_url: Some(base_url.to_string()),
                    api_key: raw
                        .get("experimental_bearer_token")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                        .or_else(|| {
                            raw.get("env_key")
                                .and_then(|value| value.as_str())
                                .and_then(|key| std::env::var(key).ok())
                        }),
                    enabled,
                    models: model_name
                        .map(|id| {
                            vec![ProviderModel {
                                id: id.to_string(),
                                name: Some(id.to_string()),
                                enabled: true,
                            }]
                        })
                        .unwrap_or_default(),
                    protocol: None,
                });
            }
        }
        if providers.is_empty()
            && let Some(api_base) = cfg.get("api_base").and_then(|value| value.as_str())
        {
            providers.push(ProviderData {
                name: "OpenAI".to_string(),
                base_url: Some(api_base.to_string()),
                api_key: None,
                enabled: true,
                models: current_model
                    .map(|id| {
                        vec![ProviderModel {
                            id: id.to_string(),
                            name: Some(id.to_string()),
                            enabled: true,
                        }]
                    })
                    .unwrap_or_default(),
                protocol: None,
            });
        }
        Ok(providers)
    }

    fn set_data(&self, value: ProviderData) -> SentraResult<AssetMutationResult> {
        let values = vec![value];
        let cfg = merge_codex_config(load_config(self.core.agent_home())?, &values);
        save_config(self.core.agent_home(), &cfg)?;
        ensure_catalog_entries(self.core.agent_home(), &values)?;
        save_auth_key(self.core.agent_home(), &values)?;
        Ok(AssetMutationResult::changed())
    }

    fn del_data(&self, item: &ProviderData) -> SentraResult<AssetMutationResult> {
        delete_provider_config(self.core.agent_home(), item)
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

fn merge_codex_config(mut cfg: toml::Table, data: &[ProviderData]) -> toml::Table {
    let mut providers = cfg
        .remove("model_providers")
        .and_then(|value| match value {
            toml::Value::Table(table) => Some(table),
            _ => None,
        })
        .unwrap_or_default();

    for provider in data {
        let provider_id = provider_id(provider);
        let mut entry = providers
            .remove(&provider_id)
            .and_then(|value| match value {
                toml::Value::Table(table) => Some(table),
                _ => None,
            })
            .unwrap_or_default();
        entry.insert(
            "requires_openai_auth".to_string(),
            toml::Value::Boolean(true),
        );
        entry.insert(
            "name".to_string(),
            toml::Value::String(provider_host_or_name(provider)),
        );
        if let Some(base_url) = &provider.base_url {
            entry.insert(
                "base_url".to_string(),
                toml::Value::String(base_url.clone()),
            );
        }
        if let Some(api_key) = &provider.api_key {
            entry.insert(
                "experimental_bearer_token".to_string(),
                toml::Value::String(api_key.clone()),
            );
        }
        if let Some(model) = provider.models.first() {
            cfg.insert("model".to_string(), toml::Value::String(model.id.clone()));
        }
        cfg.insert(
            "model_provider".to_string(),
            toml::Value::String(provider_id.clone()),
        );
        providers.insert(provider_id, toml::Value::Table(entry));
    }

    cfg.insert("model_providers".to_string(), toml::Value::Table(providers));
    cfg
}

fn delete_provider_config(
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
    let mut cfg = toml::from_str::<toml::Table>(&content).unwrap_or_default();
    let active_id = cfg
        .get("model_provider")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let mut removed_ids = Vec::new();
    if let Some(toml::Value::Table(providers)) = cfg.get_mut("model_providers") {
        let ids = providers
            .iter()
            .filter_map(|(id, value)| {
                let toml::Value::Table(entry) = value else {
                    return None;
                };
                provider_matches(entry, provider).then(|| id.clone())
            })
            .collect::<Vec<_>>();
        for id in &ids {
            providers.remove(id);
        }
        removed_ids = ids;
    }
    let remove_legacy_api_base = cfg
        .get("api_base")
        .and_then(|value| value.as_str())
        .zip(provider.base_url.as_deref())
        .map(|(api_base, base_url)| api_base == base_url)
        .unwrap_or(false)
        && removed_ids.is_empty();

    if removed_ids.is_empty() && !remove_legacy_api_base {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::NotFound,
            "provider was not found in codex config",
        ));
    }
    if active_id
        .as_deref()
        .is_some_and(|active| removed_ids.iter().any(|id| id == active))
        || remove_legacy_api_base
    {
        cfg.remove("model_provider");
        cfg.remove("model");
    }
    if remove_legacy_api_base {
        cfg.remove("api_base");
    }
    backup_file(&config_path)?;
    let content =
        toml::to_string_pretty(&cfg).map_err(|err| SentraError::Message(err.to_string()))?;
    write_text_file(config_path, &content)?;
    Ok(AssetMutationResult::changed())
}

fn ensure_catalog_entries(agent_home: &std::path::Path, data: &[ProviderData]) -> SentraResult<()> {
    let catalog_path = agent_home.join("cc-switch-model-catalog.json");
    let Some(mut catalog) = read_json_file(&catalog_path)? else {
        return Ok(());
    };
    let Some(models) = catalog
        .get_mut("models")
        .and_then(|value| value.as_array_mut())
    else {
        return Ok(());
    };
    let mut changed = false;
    for model in data.iter().flat_map(|provider| &provider.models) {
        if models
            .iter()
            .any(|entry| entry.get("slug").and_then(|value| value.as_str()) == Some(&model.id))
        {
            continue;
        }
        models.push(catalog_model(model));
        changed = true;
    }
    if changed {
        backup_file(&catalog_path)?;
        write_json_file(catalog_path, &catalog)?;
    }
    Ok(())
}

fn save_auth_key(agent_home: &std::path::Path, data: &[ProviderData]) -> SentraResult<()> {
    let Some(api_key) = data
        .first()
        .and_then(|provider| provider.api_key.as_deref())
    else {
        return Ok(());
    };
    let auth_path = agent_home.join("auth.json");
    let mut auth = read_json_file(&auth_path)?.unwrap_or_else(|| json!({}));
    if !auth.is_object() {
        auth = json!({});
    }
    auth["OPENAI_API_KEY"] = json!(api_key);
    backup_file(&auth_path)?;
    write_json_file(auth_path, &auth)
}

fn provider_matches(entry: &toml::Table, provider: &ProviderData) -> bool {
    let base_url = entry.get("base_url").and_then(|value| value.as_str());
    if base_url != provider.base_url.as_deref() {
        return false;
    }
    if let Some(api_key) = &provider.api_key {
        entry
            .get("experimental_bearer_token")
            .and_then(|value| value.as_str())
            == Some(api_key)
    } else {
        true
    }
}

fn provider_id(provider: &ProviderData) -> String {
    provider
        .base_url
        .as_deref()
        .and_then(host_from_url)
        .unwrap_or_else(|| provider.name.clone())
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn provider_host_or_name(provider: &ProviderData) -> String {
    provider
        .base_url
        .as_deref()
        .and_then(host_from_url)
        .unwrap_or_else(|| provider.name.clone())
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

fn catalog_model(model: &ProviderModel) -> serde_json::Value {
    json!({
        "slug": model.id,
        "display_name": model.name.as_deref().unwrap_or(&model.id),
        "context_window": 131072,
        "input_modalities": ["text"],
        "shell_type": "shell_command",
        "supported_in_api": true,
        "supports_parallel_tool_calls": true,
        "supported_reasoning_levels": [],
        "visibility": "list",
        "priority": 100
    })
}

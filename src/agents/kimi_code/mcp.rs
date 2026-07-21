use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData, McpType};
use crate::utils::{read_json_file, sanitize_mcp_data};

#[derive(Debug, Clone)]
pub(super) struct McpAsset {
    pub(crate) core: AssetCore,
}

impl McpAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(McpAsset, AssetType::Mcp, Vec<McpData>);

impl Asset<Vec<McpData>> for McpAsset {
    fn get_data(&self) -> SentraResult<Vec<McpData>> {
        mcp_data(self.core.agent_home())
    }
}

fn mcp_data(agent_home: &std::path::Path) -> SentraResult<Vec<McpData>> {
    let mut results = Vec::new();
    if let Some(config) = read_json_file(agent_home.join("mcp.json"))? {
        results.extend(parse_kimi_mcp_servers(
            config.get("mcpServers").unwrap_or(&Value::Null),
        ));
    }
    for manifest in crate::agents::kimi_code::plugin::plugin_manifests(agent_home)? {
        if manifest.enabled {
            results.extend(parse_kimi_mcp_servers(
                manifest.value.get("mcpServers").unwrap_or(&Value::Null),
            ));
        }
    }
    Ok(dedup_mcp(results))
}

fn parse_kimi_mcp_servers(raw: &Value) -> Vec<McpData> {
    let Some(map) = raw.as_object() else {
        return Vec::new();
    };
    map.iter()
        .map(|(name, server)| {
            let value = server.as_object();
            let (command, mut args) = value
                .and_then(|value| value.get("command"))
                .map_or((None, Vec::new()), command_parts);
            args.extend(
                value
                    .and_then(|value| value.get("args"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.as_str().map(str::to_string)),
            );
            let url = value
                .and_then(|value| value.get("url"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let mut data = McpData {
                name: name.clone(),
                mcp_type: Some(mcp_type(value, command.is_some(), url.is_some())),
                command,
                args,
                url,
                env: env_map(value),
                enabled: Some(enabled(value)),
                project: None,
            };
            sanitize_mcp_data(&mut data);
            data
        })
        .collect()
}

fn command_parts(raw: &Value) -> (Option<String>, Vec<String>) {
    if let Some(command) = raw.as_str() {
        return (Some(command.to_string()), Vec::new());
    }
    let Some(items) = raw.as_array() else {
        return (None, Vec::new());
    };
    let mut parts = items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_string));
    let command = parts.next();
    (command, parts.collect())
}

fn mcp_type(
    value: Option<&serde_json::Map<String, Value>>,
    has_command: bool,
    has_url: bool,
) -> McpType {
    let transport = value
        .and_then(|value| value.get("transport").or_else(|| value.get("type")))
        .and_then(Value::as_str);
    match transport {
        Some("stdio") => McpType::Stdio,
        Some("sse") => McpType::Sse,
        Some("http" | "streamable_http") => McpType::Http,
        _ if has_command => McpType::Stdio,
        _ if has_url => McpType::Http,
        _ => McpType::Stdio,
    }
}

fn env_map(value: Option<&serde_json::Map<String, Value>>) -> Option<HashMap<String, String>> {
    value
        .and_then(|value| value.get("env"))
        .and_then(Value::as_object)
        .map(|env| {
            env.iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
}

fn enabled(value: Option<&serde_json::Map<String, Value>>) -> bool {
    let Some(value) = value else {
        return true;
    };
    if value
        .get("disabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return false;
    }
    value
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn dedup_mcp(items: Vec<McpData>) -> Vec<McpData> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for item in items {
        if seen.insert(item.name.clone()) {
            results.push(item);
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use crate::agents::kimi_code::mcp::parse_kimi_mcp_servers;

    #[test]
    fn redacts_custom_mcp_secrets() {
        let servers = parse_kimi_mcp_servers(&serde_json::json!({
            "example": {
                "command": "server",
                "args": ["--api-key=argument-secret"],
                "env": { "ACCESS_TOKEN": "env-secret" }
            }
        }));

        assert_eq!(servers[0].args, ["--api-key=****"]);
        assert_eq!(servers[0].env.as_ref().unwrap()["ACCESS_TOKEN"], "****");
    }
}

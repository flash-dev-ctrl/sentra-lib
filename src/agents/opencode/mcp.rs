use std::collections::HashMap;

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
    for path in crate::agents::opencode::config_files(agent_home) {
        let Some(config) = read_json_file(path)? else {
            continue;
        };
        let raw = config
            .get("mcp")
            .and_then(|mcp| mcp.get("servers").or(Some(mcp)))
            .or_else(|| config.get("mcpServers"))
            .unwrap_or(&Value::Null);
        for item in parse_opencode_mcp_servers(raw) {
            if !results
                .iter()
                .any(|existing: &McpData| existing.name == item.name)
            {
                results.push(item);
            }
        }
    }
    Ok(results)
}

fn parse_opencode_mcp_servers(raw: &Value) -> Vec<McpData> {
    let Some(map) = raw.as_object() else {
        return Vec::new();
    };
    map.iter()
        .map(|(name, server)| {
            let value = server.as_object();
            let (command, command_args) = value
                .and_then(|value| value.get("command"))
                .map_or((None, Vec::new()), command_parts);
            let url = value
                .and_then(|value| value.get("url"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let mut args = command_args;
            args.extend(
                value
                    .and_then(|value| value.get("args"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.as_str().map(str::to_string)),
            );
            let mut data = McpData {
                name: name.clone(),
                mcp_type: mcp_type(
                    value.and_then(|value| value.get("type")),
                    command.is_some(),
                    url.as_deref(),
                ),
                command,
                args,
                url,
                env: value
                    .and_then(|value| value.get("environment").or_else(|| value.get("env")))
                    .and_then(Value::as_object)
                    .map(|env| {
                        env.iter()
                            .filter_map(|(key, value)| {
                                value.as_str().map(|value| (key.clone(), value.to_string()))
                            })
                            .collect::<HashMap<_, _>>()
                    }),
                enabled: Some(enabled(value)),
                project: None,
            };
            sanitize_mcp_data(&mut data);
            data
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::agents::opencode::mcp::parse_opencode_mcp_servers;

    #[test]
    fn redacts_custom_mcp_secrets() {
        let servers = parse_opencode_mcp_servers(&serde_json::json!({
            "example": {
                "command": ["server", "--token", "command-secret"],
                "args": ["--header", "Authorization: Bearer header-secret"],
                "environment": { "API_KEY": "env-secret", "REGION": "local" }
            }
        }));
        let server = &servers[0];

        assert_eq!(
            server.args,
            ["--token", "****", "--header", "Authorization:****"]
        );
        assert_eq!(server.env.as_ref().unwrap()["API_KEY"], "****");
        assert_eq!(server.env.as_ref().unwrap()["REGION"], "local");
    }
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

fn mcp_type(raw: Option<&Value>, has_command: bool, url: Option<&str>) -> Option<McpType> {
    match raw.and_then(Value::as_str) {
        Some("local" | "stdio") => Some(McpType::Stdio),
        Some("http") => Some(McpType::Http),
        Some("sse") => Some(McpType::Sse),
        Some("remote") => {
            if url.is_some_and(|value| value.to_ascii_lowercase().contains("sse")) {
                Some(McpType::Sse)
            } else {
                Some(McpType::Http)
            }
        }
        _ if has_command => Some(McpType::Stdio),
        _ if url.is_some() => Some(McpType::Http),
        _ => Some(McpType::Stdio),
    }
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

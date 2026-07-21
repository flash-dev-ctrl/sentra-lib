use std::collections::HashMap;

use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData, McpType};
use crate::utils::{read_json_file, sanitize_mcp_data};

#[derive(Debug, Clone)]
pub(super) struct McpAsset {
    core: AssetCore,
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
        let mut out = Vec::new();
        for file in ["mcp.json", "settings.json"] {
            let Some(config) = read_json_file(self.core.agent_home().join(file))? else {
                continue;
            };
            out.extend(parse_servers(
                config.get("mcpServers").or_else(|| config.get("mcp")),
            ));
        }
        Ok(out)
    }
}

fn parse_servers(raw: Option<&Value>) -> Vec<McpData> {
    let Some(map) = raw.and_then(Value::as_object) else {
        return Vec::new();
    };
    map.iter()
        .map(|(name, server)| {
            let raw = server.as_object();
            let mut data = McpData {
                name: name.clone(),
                mcp_type: Some(if string(raw, "url").is_some() {
                    McpType::Http
                } else {
                    McpType::Stdio
                }),
                command: string(raw, "command"),
                args: raw
                    .and_then(|raw| raw.get("args"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect(),
                url: string(raw, "url"),
                env: env(raw),
                enabled: Some(
                    !raw.and_then(|raw| raw.get("disabled"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                ),
                project: None,
            };
            sanitize_mcp_data(&mut data);
            data
        })
        .collect()
}

fn string(raw: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<String> {
    raw?.get(key)?.as_str().map(str::to_string)
}

fn env(raw: Option<&serde_json::Map<String, Value>>) -> Option<HashMap<String, String>> {
    let map = raw?.get("env")?.as_object()?;
    Some(
        map.iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_custom_mcp_secrets() {
        let servers = parse_servers(Some(&serde_json::json!({
            "example": {
                "command": "server",
                "args": ["--password", "argument-secret"],
                "env": { "CLIENT_SECRET": "env-secret" }
            }
        })));

        assert_eq!(servers[0].args, ["--password", "****"]);
        assert_eq!(servers[0].env.as_ref().unwrap()["CLIENT_SECRET"], "****");
    }
}

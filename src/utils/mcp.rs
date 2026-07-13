use std::collections::HashMap;

use crate::interfaces::{McpData, McpType};

pub fn parse_mcp_servers(raw: &serde_json::Value, project: Option<String>) -> Vec<McpData> {
    let Some(map) = raw.as_object() else {
        return Vec::new();
    };
    map.iter()
        .map(|(name, server)| {
            let value = server.as_object();
            let explicit_type = value
                .and_then(|v| v.get("type"))
                .and_then(|v| v.as_str())
                .and_then(|value| match value {
                    "stdio" => Some(McpType::Stdio),
                    "sse" => Some(McpType::Sse),
                    "http" => Some(McpType::Http),
                    _ => None,
                });
            let command = value
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let url = value
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let mcp_type = explicit_type.or_else(|| {
                if url.is_some() {
                    Some(McpType::Sse)
                } else {
                    Some(McpType::Stdio)
                }
            });
            let args = value
                .and_then(|v| v.get("args"))
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let env = value
                .and_then(|v| v.get("env"))
                .and_then(|v| v.as_object())
                .map(|env| {
                    env.iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|value| (key.clone(), value.to_string()))
                        })
                        .collect::<HashMap<_, _>>()
                });
            let disabled = value
                .and_then(|v| v.get("disabled"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let enabled_false = value
                .and_then(|v| v.get("enabled"))
                .and_then(|v| v.as_bool())
                .map(|enabled| !enabled)
                .unwrap_or(false);
            McpData {
                name: name.clone(),
                mcp_type,
                command,
                args,
                url,
                env,
                enabled: Some(!disabled && !enabled_false),
                project: project.clone(),
            }
        })
        .collect()
}

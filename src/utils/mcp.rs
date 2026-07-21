use std::collections::HashMap;

use crate::interfaces::{McpData, McpType};
use crate::utils::{sanitize_command_args, sanitize_env_value, sanitize_url_credentials};

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
                        .collect::<Vec<_>>()
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
            let mut data = McpData {
                name: name.clone(),
                mcp_type,
                command,
                args,
                url,
                env,
                enabled: Some(!disabled && !enabled_false),
                project: project.clone(),
            };
            sanitize_mcp_data(&mut data);
            data
        })
        .collect()
}

pub(crate) fn sanitize_mcp_data(data: &mut McpData) {
    data.args = sanitize_command_args(&data.args);
    if let Some(url) = &mut data.url {
        *url = sanitize_url_credentials(url);
    }
    if let Some(env) = &mut data.env {
        for (key, value) in env {
            *value = sanitize_env_value(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_environment_and_arguments() {
        let servers = parse_mcp_servers(
            &serde_json::json!({
                "example": {
                    "command": "server",
                    "args": ["--token", "token-value", "--api-key=key-value", "--verbose"],
                    "url": "https://user:password@mcp.example/mcp?token=url-token&format=json&monkey=banana&author=alice&keyboard=us&X-Amz-Signature=signed#api%5Fkey=fragment-secret",
                    "env": {
                        "API_KEY": "env-key-value",
                        "REGION": "us-east-1"
                    }
                }
            }),
            None,
        );
        let server = &servers[0];

        assert_eq!(
            server.args,
            ["--token", "****", "--api-key=****", "--verbose"]
        );
        assert_eq!(
            server.url.as_deref(),
            Some(
                "https://****@mcp.example/mcp?token=****&format=json&monkey=banana&author=alice&keyboard=us&X-Amz-Signature=****#api%5Fkey=****"
            )
        );
        let env = server.env.as_ref().unwrap();
        assert_eq!(env.get("API_KEY").map(String::as_str), Some("****"));
        assert_eq!(env.get("REGION").map(String::as_str), Some("us-east-1"));
    }
}

use std::collections::HashMap;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData, McpType};
use crate::utils::{read_text_file, sanitize_mcp_data};

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
        let cwd = std::env::current_dir().unwrap_or_default();
        for sock in [
            self.core.agent_home().join("mcp.sock"),
            cwd.join(".marvis").join("mcp.sock"),
        ] {
            if !sock.exists() {
                continue;
            }
            out.push(McpData {
                name: "marvis-mcp".to_string(),
                mcp_type: Some(McpType::Stdio),
                command: Some("marvis-mcp".to_string()),
                args: Vec::new(),
                url: None,
                env: None,
                enabled: Some(true),
                project: None,
            });
        }
        for settings in [
            self.core.agent_home().join("settings.yaml"),
            cwd.join(".marvis").join("settings.yaml"),
        ] {
            if let Some(config) = read_yaml(settings)? {
                let root = config.as_mapping();
                out.extend(parse_servers(
                    yaml_field(root, "mcpServers").or_else(|| yaml_field(root, "mcp_servers")),
                ));
            }
        }
        Ok(out)
    }
}

fn read_yaml(path: impl AsRef<std::path::Path>) -> SentraResult<Option<serde_yaml::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    serde_yaml::from_str(&content).map(Some).map_err(Into::into)
}

fn parse_servers(raw: Option<&serde_yaml::Value>) -> Vec<McpData> {
    let Some(map) = raw.and_then(serde_yaml::Value::as_mapping) else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(name, server)| {
            let name = name.as_str()?.to_string();
            let value = server.as_mapping();
            let mut data = McpData {
                name,
                mcp_type: Some(if string(value, "url").is_some() {
                    McpType::Http
                } else {
                    McpType::Stdio
                }),
                command: string(value, "command"),
                args: array(value, "args"),
                url: string(value, "url"),
                env: env(value),
                enabled: Some(!bool_field(value, "disabled").unwrap_or(false)),
                project: None,
            };
            sanitize_mcp_data(&mut data);
            Some(data)
        })
        .collect()
}

fn yaml_field<'a>(
    raw: Option<&'a serde_yaml::Mapping>,
    key: &str,
) -> Option<&'a serde_yaml::Value> {
    raw?.get(&serde_yaml::Value::String(key.to_string()))
}

fn string(raw: Option<&serde_yaml::Mapping>, key: &str) -> Option<String> {
    yaml_field(raw, key)?.as_str().map(str::to_string)
}

fn array(raw: Option<&serde_yaml::Mapping>, key: &str) -> Vec<String> {
    yaml_field(raw, key)
        .and_then(serde_yaml::Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}

fn env(raw: Option<&serde_yaml::Mapping>) -> Option<HashMap<String, String>> {
    let map = yaml_field(raw, "env")?.as_mapping()?;
    Some(
        map.iter()
            .filter_map(|(key, value)| {
                Some((key.as_str()?.to_string(), value.as_str()?.to_string()))
            })
            .collect(),
    )
}

fn bool_field(raw: Option<&serde_yaml::Mapping>, key: &str) -> Option<bool> {
    yaml_field(raw, key)?.as_bool()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_custom_mcp_secrets() {
        let config: serde_yaml::Value = serde_yaml::from_str(
            r#"
example:
  command: server
  args: [--token, argument-secret]
  env:
    API_KEY: env-secret
"#,
        )
        .unwrap();
        let servers = parse_servers(Some(&config));

        assert_eq!(servers[0].args, ["--token", "****"]);
        assert_eq!(servers[0].env.as_ref().unwrap()["API_KEY"], "****");
    }
}

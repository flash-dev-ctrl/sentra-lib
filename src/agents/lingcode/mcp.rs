use crate::SentraResult;
use crate::agents::install_status::hidden_home_parent;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData};
use crate::utils::{mask_secret, parse_mcp_servers, read_json_file};

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
        let home = self.core.agent_home();
        let claude_home = hidden_home_parent(home).join(".claude");
        let mut results = Vec::new();
        for path in [home.join("mcp.json"), claude_home.join(".claude.json")] {
            let Some(config) = read_json_file(path)? else {
                continue;
            };
            results.extend(parse_mcp_servers(
                config.get("mcpServers").unwrap_or(&serde_json::Value::Null),
                None,
            ));
            if let Some(projects) = config.get("projects").and_then(|value| value.as_object()) {
                for (project, value) in projects {
                    results.extend(parse_mcp_servers(
                        value.get("mcpServers").unwrap_or(&serde_json::Value::Null),
                        Some(project.clone()),
                    ));
                }
            }
        }
        mask_mcp_env(&mut results);
        Ok(results)
    }
}

fn mask_mcp_env(servers: &mut [McpData]) {
    for server in servers {
        if let Some(env) = &mut server.env {
            for (key, value) in env {
                if is_sensitive_key(key) {
                    *value = mask_secret(Some(value)).unwrap_or_default();
                }
            }
        }
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["key", "token", "password", "secret"]
        .iter()
        .any(|part| key.contains(part))
}

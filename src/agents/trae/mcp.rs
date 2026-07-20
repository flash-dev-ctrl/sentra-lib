use crate::SentraResult;
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
        let mut results = Vec::new();
        if let Some(path) = crate::agents::trae::workspace_path(".trae/mcp.json") {
            if let Some(config) = read_json_file(path)? {
                results.extend(parse_mcp_servers(
                    config
                        .get("mcpServers")
                        .or_else(|| config.get("servers"))
                        .unwrap_or(&serde_json::Value::Null),
                    None,
                ));
            }
        }
        mask_env(&mut results);
        Ok(results)
    }
}

fn mask_env(servers: &mut [McpData]) {
    for server in servers {
        if let Some(env) = &mut server.env {
            for (key, value) in env {
                let key = key.to_ascii_lowercase();
                if ["key", "token", "password", "secret"]
                    .iter()
                    .any(|part| key.contains(part))
                {
                    *value = mask_secret(Some(value)).unwrap_or_default();
                }
            }
        }
    }
}

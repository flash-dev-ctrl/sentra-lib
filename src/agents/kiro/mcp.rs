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
        let Some(config) =
            read_json_file(self.core.agent_home().join("settings").join("mcp.json"))?
        else {
            return Ok(Vec::new());
        };
        let mut servers = parse_mcp_servers(
            config.get("mcpServers").unwrap_or(&serde_json::Value::Null),
            None,
        );
        for server in &mut servers {
            if let Some(env) = &mut server.env {
                for (key, value) in env {
                    if is_sensitive_key(key) {
                        *value = mask_secret(Some(value)).unwrap_or_default();
                    }
                }
            }
        }
        Ok(servers)
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["key", "token", "password", "secret"]
        .iter()
        .any(|part| key.contains(part))
}

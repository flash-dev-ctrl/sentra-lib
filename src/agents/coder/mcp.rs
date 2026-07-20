use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, McpData};
use crate::utils::{parse_mcp_servers, read_json_file};
use crate::SentraResult;

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
        let mut results = Vec::new();
        for path in [
            std::env::current_dir()
                .unwrap_or_default()
                .join(".mcp.json"),
            super::config_home(self.core.agent_home()).join("mcp.json"),
        ] {
            let Some(config) = read_json_file(path)? else {
                continue;
            };
            results.extend(parse_mcp_servers(
                config
                    .get("mcpServers")
                    .or_else(|| config.get("servers"))
                    .unwrap_or(&serde_json::Value::Null),
                None,
            ));
        }
        Ok(results)
    }
}

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData};
use crate::utils::{parse_mcp_servers, read_json_file};

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
    let Some(config) = read_json_file(agent_home.join("openclaw.json"))? else {
        return Ok(Vec::new());
    };
    let raw = config
        .get("mcp")
        .and_then(|mcp| mcp.get("servers"))
        .or_else(|| config.get("mcpServers"))
        .unwrap_or(&serde_json::Value::Null);
    Ok(parse_mcp_servers(raw, None))
}

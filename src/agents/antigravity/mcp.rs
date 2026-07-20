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
        let mut results = Vec::new();
        for path in [
            super::user_home(self.core.agent_home())
                .join(".gemini")
                .join("config")
                .join("mcp_config.json"),
            self.core.agent_home().join("mcp_config.json"),
            std::env::current_dir()
                .unwrap_or_default()
                .join(".agents")
                .join("mcp_config.json"),
        ] {
            let Some(config) = read_json_file(path)? else {
                continue;
            };
            let raw = config
                .get("mcpServers")
                .or_else(|| config.get("servers"))
                .unwrap_or(&serde_json::Value::Null);
            results.extend(parse_mcp_servers(raw, None));
        }
        Ok(results)
    }
}

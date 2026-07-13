use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData};
use crate::utils::{parse_mcp_servers, read_text_file};

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
        let Some(content) = read_text_file(self.core.agent_home().join("config.toml"))? else {
            return Ok(Vec::new());
        };
        let Ok(value) = toml::from_str::<toml::Value>(&content) else {
            return Ok(Vec::new());
        };
        let json = serde_json::to_value(value).unwrap_or_default();
        Ok(parse_mcp_servers(
            json.get("mcp_servers").unwrap_or(&serde_json::Value::Null),
            None,
        ))
    }
}

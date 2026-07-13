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
        mcp_data(self.core.agent_home())
    }
}

fn mcp_data(agent_home: &std::path::Path) -> SentraResult<Vec<McpData>> {
    let Some(config) = read_yaml_file(&agent_home.join("config.yaml"))? else {
        return Ok(Vec::new());
    };
    let json = serde_json::to_value(config).unwrap_or_default();
    Ok(parse_mcp_servers(
        json.get("mcp_servers").unwrap_or(&serde_json::Value::Null),
        None,
    ))
}

fn read_yaml_file(path: &std::path::Path) -> SentraResult<Option<serde_yaml::Value>> {
    let Some(content) = read_text_file(path)? else {
        return Ok(None);
    };
    serde_yaml::from_str(&content).map(Some).map_err(Into::into)
}

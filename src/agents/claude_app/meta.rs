use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    pub(crate) core: AssetCore,
}

impl MetaAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(MetaAsset, AssetType::Meta, Option<MetaData>);

impl Asset<Option<MetaData>> for MetaAsset {
    fn get_data(&self) -> SentraResult<Option<MetaData>> {
        meta_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn meta_data(agent_name: &str, agent_home: &std::path::Path) -> SentraResult<Option<MetaData>> {
    if !dir_exists(agent_home) {
        return Ok(None);
    }
    Ok(Some(MetaData {
        id: Some(agent_name.to_string()),
        name: agent_name.to_string(),
        description: Some(
            "Anthropic's native desktop application with MCP server support.".to_string(),
        ),
        version: None,
        author: Some("Anthropic".to_string()),
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

use std::path::Path;

use crate::agents::install_status::{is_named_cli_agent_installed_with, InstallStatusProbe};
use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;
use crate::SentraResult;

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    core: AssetCore,
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
        let installed = is_agent_installed(self.core.agent_name(), self.core.agent_home());
        if !dir_exists(self.core.agent_home()) && !installed {
            return Ok(None);
        }
        Ok(Some(MetaData {
            id: Some("codebuddy".to_string()),
            name: "CodeBuddy".to_string(),
            description: None,
            version: None,
            author: Some("Tencent Cloud".to_string()),
            installed,
            home: Some(self.core.agent_home().to_path_buf()),
            created_at: None,
            updated_at: None,
        }))
    }
}

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    is_named_cli_agent_installed_with(agent_name, agent_home, &InstallStatusProbe::real())
}

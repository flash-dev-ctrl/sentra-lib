use std::path::{Path, PathBuf};

use crate::agents::install_status::{
    any_command_exists_with, any_existing_dir_with, InstallStatusProbe,
};
use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;
use crate::SentraResult;

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    pub(crate) core: AssetCore,
}

impl MetaAsset {
    pub(super) fn new(agent_name: impl Into<String>, agent_home: impl Into<PathBuf>) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(MetaAsset, AssetType::Meta, Option<MetaData>);

impl Asset<Option<MetaData>> for MetaAsset {
    fn get_data(&self) -> SentraResult<Option<MetaData>> {
        let home = super::config_home(self.core.agent_home());
        let installed = is_agent_installed(self.core.agent_name(), &home);
        if !dir_exists(&home) && !installed {
            return Ok(None);
        }
        Ok(Some(MetaData {
            id: Some(self.core.agent_name().to_string()),
            name: self.core.agent_name().to_string(),
            description: Some("Coder and code-server agent configuration metadata.".to_string()),
            version: None,
            author: Some("Coder".to_string()),
            installed,
            home: Some(home),
            created_at: None,
            updated_at: None,
        }))
    }
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real();
    any_command_exists_with(&["coder", "code-server"], &probe)
        || any_existing_dir_with(vec![super::config_home(agent_home)], &probe)
}

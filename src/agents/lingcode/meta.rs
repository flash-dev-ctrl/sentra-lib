use std::path::{Path, PathBuf};

use crate::agents::install_status::{
    any_command_exists_with, any_existing_file_with, binary_paths, hidden_home_parent,
    InstallStatusProbe,
};
use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;
use crate::SentraResult;

#[derive(Debug, Clone)]
pub(super) struct MetaAsset {
    core: AssetCore,
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
        let home = self.core.agent_home();
        let installed = is_agent_installed(self.core.agent_name(), home);
        if !dir_exists(home) && !installed {
            return Ok(None);
        }
        Ok(Some(MetaData {
            id: Some("lingcode".to_string()),
            name: "LingCode".to_string(),
            description: Some(
                "LingCode proprietary agent configuration and Claude-compatible assets."
                    .to_string(),
            ),
            version: None,
            author: None,
            installed,
            home: Some(home.to_path_buf()),
            created_at: None,
            updated_at: None,
        }))
    }
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    is_agent_installed_with(agent_home, &InstallStatusProbe::real())
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["lingcode"], probe)
        || any_existing_file_with(lingcode_install_paths(agent_home), probe)
}

fn lingcode_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "lingcode");
    paths.extend(binary_paths(agent_home.join("bin"), "lingcode"));
    paths
}

use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_dir_with, any_existing_file_with,
    binary_paths, env_path, hidden_home_parent,
};
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;

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
            id: Some("qoderwork".to_string()),
            name: "QoderWork".to_string(),
            description: None,
            version: None,
            author: Some("Alibaba Cloud".to_string()),
            installed,
            home: Some(self.core.agent_home().to_path_buf()),
            created_at: None,
            updated_at: None,
        }))
    }
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real();
    any_command_exists_with(&["QoderWork", "qoderwork"], &probe)
        || any_existing_file_with(install_paths(agent_home), &probe)
        || any_existing_dir_with(app_paths(agent_home), &probe)
        || probe.product_installed(&["QoderWork"], &["Qoder", "Alibaba"])
}

fn install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "qoderwork");
    let local_app_data =
        env_path("LOCALAPPDATA").unwrap_or_else(|| user_home.join("AppData").join("Local"));
    paths.extend(binary_paths(
        local_app_data.join("Programs").join("QoderWork"),
        "QoderWork",
    ));
    paths
}

fn app_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    vec![
        user_home.join("Applications").join("QoderWork.app"),
        PathBuf::from("/Applications/QoderWork.app"),
    ]
}

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
            id: Some("trae".to_string()),
            name: "Trae".to_string(),
            description: Some(
                "Trae IDE and trae-agent CLI local configuration and assets.".to_string(),
            ),
            author: Some("ByteDance".to_string()),
            installed,
            home: Some(home.to_path_buf()),
            ..MetaData::default()
        }))
    }
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real(hidden_home_parent(agent_home));
    any_command_exists_with(&["trae"], &probe)
        || any_existing_file_with(install_paths(agent_home), &probe)
        || any_existing_dir_with(app_paths(agent_home), &probe)
        || probe.product_installed(&["Trae"], &["SPRING (SG)", "ByteDance"])
}

fn install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let mut paths = binary_paths(
        hidden_home_parent(agent_home).join(".local").join("bin"),
        "trae",
    );
    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        paths.extend(binary_paths(
            local_app_data.join("Programs").join("Trae"),
            "Trae",
        ));
    }
    #[cfg(unix)]
    paths.extend([
        PathBuf::from("/usr/bin/trae"),
        PathBuf::from("/usr/local/bin/trae"),
        PathBuf::from("/usr/share/trae/trae"),
    ]);
    paths
}

fn app_paths(agent_home: &Path) -> Vec<PathBuf> {
    vec![
        hidden_home_parent(agent_home)
            .join("Applications")
            .join("Trae.app"),
        PathBuf::from("/Applications/Trae.app"),
        PathBuf::from("/usr/share/trae"),
        PathBuf::from("/opt/Trae"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trae_cli_alone_does_not_count_as_the_desktop_product() {
        let dir = tempfile::tempdir().unwrap();
        let probe = InstallStatusProbe::test(|binary| binary == "trae-cli", |_| false, |_| false);

        assert!(
            !(any_command_exists_with(&["trae"], &probe)
                || any_existing_file_with(install_paths(dir.path()), &probe)
                || any_existing_dir_with(app_paths(dir.path()), &probe))
        );
    }
}

use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_file_with, binary_paths, env_path,
    user_home_for_agent_home,
};
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MetaData};
use crate::utils::dir_exists;

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
    let probe = InstallStatusProbe::real(user_home_for_agent_home(
        agent_home,
        &[".config", "coderv2"],
    ));
    is_install_target_installed_with(agent_home, &probe)
        || any_command_exists_with(&["code-server"], &probe)
}

pub(super) fn is_install_target_installed(agent_home: &Path) -> bool {
    is_install_target_installed_with(
        agent_home,
        &InstallStatusProbe::real(user_home_for_agent_home(
            agent_home,
            &[".config", "coderv2"],
        )),
    )
}

fn is_install_target_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["coder"], probe)
        || any_existing_file_with(install_paths(agent_home), probe)
        || probe.product_installed(&["Coder"], &["Coder Technologies"])
}

fn install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = super::user_home(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "coder");
    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        paths.extend(binary_paths(
            local_app_data
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
            "coder",
        ));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_server_does_not_satisfy_the_coder_install_target() {
        let probe =
            InstallStatusProbe::test(|binary| binary == "code-server", |_| false, |_| false);

        assert!(!is_install_target_installed_with(
            Path::new(".config/coderv2"),
            &probe
        ));
    }
}

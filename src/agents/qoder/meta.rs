use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_file_with, binary_paths, env_path,
    hidden_home_parent,
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
        let cn = self.core.agent_name() == "qoder-cn";
        Ok(Some(MetaData {
            id: Some(self.core.agent_name().to_string()),
            name: if cn { "Qoder CN" } else { "Qoder" }.to_string(),
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

pub(super) fn is_agent_installed(agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real(hidden_home_parent(agent_home));
    any_command_exists_with(&[command(agent_name)], &probe)
        || any_existing_file_with(install_paths(agent_name, agent_home), &probe)
        || is_desktop_installed_with(agent_home, &probe)
}

pub(super) fn is_install_target_installed(agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real(hidden_home_parent(agent_home));
    if cfg!(windows) {
        is_desktop_installed_with(agent_home, &probe)
    } else {
        any_command_exists_with(&["qodercli"], &probe)
            || any_existing_file_with(install_paths("qoder", agent_home), &probe)
    }
}

fn command(agent_name: &str) -> &'static str {
    if agent_name == "qoder-cn" {
        "qoderclicn"
    } else {
        "qodercli"
    }
}

fn install_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let command = command(agent_name);
    binary_paths(
        hidden_home_parent(agent_home).join(".qoder").join("bin"),
        command,
    )
}

fn is_desktop_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_existing_file_with(desktop_install_paths(agent_home), probe)
        || probe.product_installed(&["Qoder"], &["Alibaba", "Qoder"])
}

fn desktop_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let local_app_data =
        env_path("LOCALAPPDATA").unwrap_or_else(|| user_home.join("AppData").join("Local"));
    let mut paths = binary_paths(local_app_data.join("Programs").join("Qoder"), "Qoder");
    for env_name in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env_path(env_name) {
            paths.extend(binary_paths(root.join("Qoder"), "Qoder"));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_install_path_does_not_repeat_the_binary_name() {
        let paths = install_paths("qoder", Path::new("home/.qoder"));
        let binary = if cfg!(windows) {
            "qodercli.exe"
        } else {
            "qodercli"
        };

        assert!(
            paths
                .iter()
                .any(|path| path.ends_with(Path::new(".qoder").join("bin").join(binary)))
        );
        assert!(
            !paths
                .iter()
                .any(|path| path.ends_with(Path::new("qodercli").join(binary)))
        );
    }
}

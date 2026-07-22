use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_dir_with, any_existing_file_with,
    binary_paths, env_path,
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
        let home = self.core.agent_home();
        let installed = is_agent_installed(self.core.agent_name(), home);
        if !dir_exists(home) && !installed {
            return Ok(None);
        }
        Ok(Some(MetaData {
            id: Some(self.core.agent_name().to_string()),
            name: self.core.agent_name().to_string(),
            description: Some("VS Code agent, chat skill, and MCP extension metadata.".to_string()),
            version: None,
            author: Some("Microsoft".to_string()),
            installed,
            home: Some(home.to_path_buf()),
            created_at: None,
            updated_at: None,
        }))
    }
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real(super::user_home(agent_home));
    is_agent_installed_with(agent_home, &probe)
}

pub(super) fn is_install_target_installed(agent_home: &Path) -> bool {
    is_install_target_installed_with(
        agent_home,
        &InstallStatusProbe::real(super::user_home(agent_home)),
    )
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    is_install_target_installed_with(agent_home, probe)
        || any_command_exists_with(&["code-insiders"], probe)
        || any_existing_file_with(insiders_install_paths(agent_home), probe)
        || any_existing_dir_with(insiders_app_paths(agent_home), probe)
        || probe.product_installed(
            &["Microsoft Visual Studio Code Insiders"],
            &["Microsoft Corporation"],
        )
}

fn is_install_target_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["code"], probe)
        || any_existing_file_with(stable_install_paths(agent_home), probe)
        || any_existing_dir_with(stable_app_paths(agent_home), probe)
        || probe.product_installed(
            &["Microsoft Visual Studio Code"],
            &["Microsoft Corporation"],
        )
}

fn stable_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = super::user_home(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "code");
    let local_app_data =
        env_path("LOCALAPPDATA").unwrap_or_else(|| user_home.join("AppData").join("Local"));
    paths.extend(binary_paths(
        local_app_data.join("Programs").join("Microsoft VS Code"),
        "Code",
    ));
    for env_name in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env_path(env_name) {
            paths.extend(binary_paths(root.join("Microsoft VS Code"), "Code"));
        }
    }
    paths.push(PathBuf::from("/usr/bin/code"));
    paths
}

fn insiders_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = super::user_home(agent_home);
    let local_app_data =
        env_path("LOCALAPPDATA").unwrap_or_else(|| user_home.join("AppData").join("Local"));
    let mut paths = binary_paths(
        local_app_data
            .join("Programs")
            .join("Microsoft VS Code Insiders"),
        "Code - Insiders",
    );
    for env_name in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env_path(env_name) {
            paths.extend(binary_paths(
                root.join("Microsoft VS Code Insiders"),
                "Code - Insiders",
            ));
        }
    }
    paths.push(PathBuf::from("/usr/bin/code-insiders"));
    paths
}

fn stable_app_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = super::user_home(agent_home);
    vec![
        user_home
            .join("Applications")
            .join("Visual Studio Code.app"),
        PathBuf::from("/Applications/Visual Studio Code.app"),
        PathBuf::from("/usr/share/code"),
    ]
}

fn insiders_app_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = super::user_home(agent_home);
    vec![
        user_home
            .join("Applications")
            .join("Visual Studio Code - Insiders.app"),
        PathBuf::from("/Applications/Visual Studio Code - Insiders.app"),
        PathBuf::from("/usr/share/code-insiders"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insiders_does_not_satisfy_the_stable_install_target() {
        let probe =
            InstallStatusProbe::test(|binary| binary == "code-insiders", |_| false, |_| false);

        assert!(is_agent_installed_with(Path::new(".vscode"), &probe));
        assert!(!is_install_target_installed_with(
            Path::new(".vscode"),
            &probe
        ));
    }
}

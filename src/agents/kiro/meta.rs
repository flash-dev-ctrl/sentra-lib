use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_dir_with, any_existing_file_with,
    binary_paths, env_path, hidden_home_parent, user_home_for_agent_home,
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
            id: Some("kiro".to_string()),
            name: "Kiro".to_string(),
            description: Some("Kiro proprietary agent configuration and local assets.".to_string()),
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
    is_agent_installed_with(
        agent_home,
        &InstallStatusProbe::real(user_home_for_agent_home(agent_home, &[".kiro"])),
    )
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["kiro"], probe)
        || any_existing_file_with(kiro_install_paths(agent_home), probe)
        || any_existing_dir_with(kiro_app_paths(agent_home), probe)
        || probe.product_installed(&["Kiro"], &["Amazon Web Services", "Amazon"])
}

fn kiro_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "kiro");
    let local_app_data =
        env_path("LOCALAPPDATA").unwrap_or_else(|| user_home.join("AppData").join("Local"));
    paths.extend(binary_paths(
        local_app_data.join("Programs").join("Kiro"),
        "Kiro",
    ));
    for env_name in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env_path(env_name) {
            paths.extend(binary_paths(root.join("Kiro"), "Kiro"));
        }
    }
    #[cfg(unix)]
    paths.extend([
        PathBuf::from("/usr/bin/kiro"),
        PathBuf::from("/usr/local/bin/kiro"),
        PathBuf::from("/usr/share/kiro/kiro"),
        PathBuf::from("/opt/Kiro/kiro"),
        PathBuf::from("/opt/kiro/kiro"),
    ]);
    paths
}

fn kiro_app_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    vec![
        user_home.join("Applications").join("Kiro.app"),
        PathBuf::from("/Applications/Kiro.app"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_install_paths_are_accepted_without_cli() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".kiro");
        let executable_probe = InstallStatusProbe::test(
            command_never_exists,
            only_standard_desktop_executable,
            path_never_exists,
        );
        let bundle_probe = InstallStatusProbe::test(
            command_never_exists,
            path_never_exists,
            only_kiro_bundle_exists,
        );

        assert!(is_agent_installed_with(&home, &executable_probe));
        assert!(is_agent_installed_with(&home, &bundle_probe));
    }

    #[test]
    fn kiro_cli_alone_does_not_count_as_the_desktop_product() {
        let dir = tempfile::tempdir().unwrap();
        let probe = InstallStatusProbe::test(|binary| binary == "kiro-cli", |_| false, |_| false);

        assert!(!is_agent_installed_with(&dir.path().join(".kiro"), &probe));
    }

    #[cfg(unix)]
    #[test]
    fn includes_linux_system_executable_paths() {
        let paths = kiro_install_paths(Path::new("home/.kiro"));

        assert!(paths.contains(&PathBuf::from("/usr/share/kiro/kiro")));
        assert!(paths.contains(&PathBuf::from("/opt/Kiro/kiro")));
    }

    fn command_never_exists(_: &str) -> bool {
        false
    }

    fn path_never_exists(_: &Path) -> bool {
        false
    }

    fn only_standard_desktop_executable(path: &Path) -> bool {
        let binary = if cfg!(windows) { "Kiro.exe" } else { "Kiro" };
        path.ends_with(Path::new("Programs").join("Kiro").join(binary))
    }

    fn only_kiro_bundle_exists(path: &Path) -> bool {
        path.ends_with("Kiro.app")
    }
}

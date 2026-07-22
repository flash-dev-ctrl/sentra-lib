use std::path::{Path, PathBuf};

use crate::SentraResult;
use crate::agents::install_status::{
    InstallStatusProbe, any_command_exists_with, any_existing_file_with, binary_paths,
    hidden_home_parent,
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
            description: Some("Google Antigravity agent CLI configuration.".to_string()),
            version: None,
            author: Some("Google".to_string()),
            installed,
            home: Some(home.to_path_buf()),
            created_at: None,
            updated_at: None,
        }))
    }
}

pub(super) fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real(antigravity_user_home(agent_home));
    is_agent_installed_with(agent_home, &probe)
}

pub(super) fn is_install_target_installed(agent_home: &Path) -> bool {
    is_install_target_installed_with(
        agent_home,
        &InstallStatusProbe::real(antigravity_user_home(agent_home)),
    )
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    is_install_target_installed_with(agent_home, probe)
        || any_existing_file_with(binary_paths(agent_home.join("bin"), "agy"), probe)
        || any_command_exists_with(&["Antigravity"], probe)
}

fn is_install_target_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["agy"], probe)
        || any_existing_file_with(official_install_paths(agent_home), probe)
        || probe.product_installed(&["Antigravity CLI"], &["Google"])
}

fn official_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    binary_paths(
        antigravity_user_home(agent_home).join(".local").join("bin"),
        "agy",
    )
}

fn antigravity_user_home(agent_home: &Path) -> PathBuf {
    agent_home
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| hidden_home_parent(agent_home))
}

#[cfg(test)]
mod tests {
    use super::{is_agent_installed_with, is_install_target_installed_with};
    use crate::agents::install_status::InstallStatusProbe;
    use std::path::Path;

    #[test]
    fn install_probe_accepts_agy_command() {
        let dir = tempfile::tempdir().unwrap();
        let probe = InstallStatusProbe::test(|binary| binary == "agy", |_| false, |_| false);

        assert!(is_agent_installed_with(dir.path(), &probe));
    }

    #[test]
    fn install_probe_accepts_official_user_local_binary() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".gemini").join("antigravity-cli");
        let expected = dir
            .path()
            .join(".local")
            .join("bin")
            .join(if cfg!(windows) { "agy.exe" } else { "agy" });
        std::fs::create_dir_all(expected.parent().unwrap()).unwrap();
        std::fs::write(expected, "").unwrap();
        let probe = InstallStatusProbe::test(|_| false, Path::is_file, |_| false);

        assert!(is_agent_installed_with(&agent_home, &probe));
    }

    #[test]
    fn configuration_directory_alone_is_not_an_install() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".gemini").join("antigravity-cli");
        let probe = InstallStatusProbe::test(
            |_| false,
            |_| false,
            |path| path.ends_with(Path::new(".gemini").join("antigravity-cli")),
        );

        assert!(!is_agent_installed_with(&agent_home, &probe));
    }

    #[test]
    fn desktop_command_does_not_satisfy_the_cli_install_target() {
        let probe =
            InstallStatusProbe::test(|binary| binary == "Antigravity", |_| false, |_| false);

        assert!(!is_install_target_installed_with(
            Path::new(".gemini/antigravity-cli"),
            &probe
        ));
    }

    #[test]
    fn bundled_binary_does_not_satisfy_the_official_install_target() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".gemini").join("antigravity-cli");
        let binary = agent_home
            .join("bin")
            .join(if cfg!(windows) { "agy.exe" } else { "agy" });
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(binary, "").unwrap();
        let probe = InstallStatusProbe::test(|_| false, Path::is_file, |_| false);

        assert!(is_agent_installed_with(&agent_home, &probe));
        assert!(!is_install_target_installed_with(&agent_home, &probe));
    }
}

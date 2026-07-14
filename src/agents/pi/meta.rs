use std::ffi::OsStr;
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
    pub(crate) core: AssetCore,
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
        meta_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn meta_data(agent_name: &str, agent_home: &std::path::Path) -> SentraResult<Option<MetaData>> {
    if !dir_exists(agent_home) {
        return Ok(None);
    }
    Ok(Some(MetaData {
        id: Some(agent_name.to_string()),
        name: "Pi".to_string(),
        description: Some("A terminal-based coding agent.".to_string()),
        version: None,
        author: Some("Mario Zechner".to_string()),
        installed: is_agent_installed(agent_name, agent_home),
        home: Some(agent_home.to_path_buf()),
        created_at: None,
        updated_at: None,
    }))
}

fn is_agent_installed(_agent_name: &str, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe::real();
    is_agent_installed_with(agent_home, &probe)
}

fn is_agent_installed_with(agent_home: &Path, probe: &InstallStatusProbe) -> bool {
    any_command_exists_with(&["pi"], probe)
        || any_existing_file_with(pi_install_paths(agent_home), probe)
}

fn pi_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = pi_user_home(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "pi");
    paths.extend(binary_paths(agent_home.join("bin"), "pi"));

    if let Some(app_data) = env_path("APPDATA") {
        paths.extend(binary_paths(app_data.join("npm"), "pi"));
    }
    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        paths.extend(binary_paths(local_app_data.join("pnpm"), "pi"));
    }

    paths
}

fn pi_user_home(agent_home: &Path) -> PathBuf {
    if agent_home.file_name() == Some(OsStr::new("agent"))
        && agent_home.parent().and_then(Path::file_name) == Some(OsStr::new(".pi"))
    {
        return agent_home
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| hidden_home_parent(agent_home));
    }
    if agent_home.file_name() == Some(OsStr::new(".pi")) {
        return hidden_home_parent(agent_home);
    }
    hidden_home_parent(agent_home)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::agents::install_status::InstallStatusProbe;
    use crate::agents::pi::meta::is_agent_installed_with;

    #[test]
    fn install_probe_accepts_command_presence() {
        let dir = tempfile::tempdir().unwrap();
        let pi_home = dir.path().join(".pi").join("agent");
        let probe =
            InstallStatusProbe::test(only_pi_command_exists, path_never_exists, path_never_exists);

        assert!(is_agent_installed_with(&pi_home, &probe));
    }

    #[test]
    fn install_probe_accepts_user_local_install_path() {
        let dir = tempfile::tempdir().unwrap();
        let pi_home = dir.path().join(".pi").join("agent");
        let bin_dir = dir.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join(test_binary_name("pi")), "").unwrap();
        let probe = InstallStatusProbe::test(command_never_exists, path_is_file, path_never_exists);

        assert!(is_agent_installed_with(&pi_home, &probe));
    }

    #[test]
    fn install_probe_requires_binary_not_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let pi_home = dir.path().join(".pi").join("agent");
        std::fs::create_dir_all(&pi_home).unwrap();
        let probe =
            InstallStatusProbe::test(command_never_exists, path_never_exists, path_never_exists);

        assert!(!is_agent_installed_with(&pi_home, &probe));
    }

    fn command_never_exists(_: &str) -> bool {
        false
    }

    fn only_pi_command_exists(binary: &str) -> bool {
        binary == "pi"
    }

    fn path_never_exists(_: &Path) -> bool {
        false
    }

    fn path_is_file(path: &Path) -> bool {
        path.is_file()
    }

    fn test_binary_name(binary: &str) -> String {
        if cfg!(windows) {
            format!("{binary}.exe")
        } else {
            binary.to_string()
        }
    }
}
